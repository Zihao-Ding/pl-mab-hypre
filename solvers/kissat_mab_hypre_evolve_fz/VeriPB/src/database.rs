use std::{cell::RefCell, ops::Range, rc::Rc};

use ahash::{HashSet, RandomState};
use by_address::ByAddress;
use veripb_formula::prelude::*;
use veripb_propagator::propagation_engine::PropagationEngine;

use crate::{
    context::{Context, CORE, DERIVED},
    error::CheckingError,
    occurrence_list::OccurrenceList,
};

const IGNORE_DATABASE_PROOFGOAL_THRESHOLD: f64 = 0.05;

/// Remove the constraint from the core propagator and add it to the derived propagator.
#[inline]
pub fn move_to_derived_propagator(
    context: &mut Context,
    constraint: &Rc<DBConstraint>,
) -> Result<(), CheckingError> {
    context.propagation_engine.detach(
        CORE,
        constraint,
        context.propagation_engine.only_core_trail,
    )?;
    context.propagation_engine.attach(DERIVED, constraint)?;
    Ok(())
}

/// Storage for constraints.
#[derive(Debug, Default)]
pub struct Database {
    /// Constraint ID indexed vector to directly get a constraint from its index.
    pub entries: Vec<Option<Rc<DBConstraint>>>,
    /// Hash map of unique constraints identified by the constraint of the entry.
    pub unique_constraints: HashSet<Rc<DBConstraint>>,
    /// An occurrence list mapping from literals to constraints. This list is kept in sync with the `unique_constraints`.
    occurrences: OccurrenceList,
    /// Only constraint IDs that are less than `next_unique_index_id` have been added to `unique_constraints`.
    next_unique_index_id: usize,
    /// Only constraint IDs that are less than `next_occurrences_index_id` have been added to `occurrences`.
    next_occurrences_index_id: usize,
    /// Only constraint IDs that are less than `next_propagation_index_id` have been added to the propagation engine.
    next_propagation_index_id: usize,
    /// The maximum size of the database to elaborate all database proofgoals even if they are autoproven.
    ignore_database_proofgoal_size: usize,
}

impl Database {
    /// Create a new empty database with dummy entry at position 0.
    pub fn new() -> Self {
        Database {
            entries: vec![None],
            ..Default::default()
        }
    }

    /// Initialize a database from the a `Formula`.
    pub fn from_formula(constraints: Vec<PBConstraintEnum>) -> Self {
        let mut entries = Vec::with_capacity(constraints.len() + 1);
        entries.push(None);
        let mut unique_constraints = HashSet::with_capacity_and_hasher(
            constraints.len(),
            RandomState::with_seeds(42, 42, 42, 42),
        );
        let mut occurrences = OccurrenceList::default();
        for constraint in constraints {
            let db_constraint = Rc::new(DBConstraint {
                header: RefCell::new(DBHeader::default()),
                constraint,
            });
            if unique_constraints.insert(Rc::clone(&db_constraint)) {
                db_constraint.add_id(entries.len(), true);
                occurrences.add(&db_constraint);
                entries.push(Some(db_constraint));
            } else {
                // Constraint already in `unique_constraints`.
                let entry = unique_constraints.get(&db_constraint).unwrap();
                entry.add_id(entries.len(), true);
                entries.push(Some(Rc::clone(entry)));
            }
        }
        let first_non_indexed_constraint_id = entries.len();
        let ignore_database_proofgoal_size =
            (IGNORE_DATABASE_PROOFGOAL_THRESHOLD * (entries.len() as f64)) as usize;

        Database {
            entries,
            unique_constraints,
            occurrences,
            next_unique_index_id: first_non_indexed_constraint_id,
            next_occurrences_index_id: first_non_indexed_constraint_id,
            next_propagation_index_id: first_non_indexed_constraint_id,
            ignore_database_proofgoal_size,
        }
    }

    /// Lazily adds a constraint to the database.
    ///
    /// If the constraint is already in the database, then it will be added lazily again.
    #[inline]
    pub fn add_constraint(&mut self, constraint: Rc<DBConstraint>, add_to_core: bool) {
        constraint.add_id(self.len(), add_to_core);
        self.entries.push(Some(constraint));
    }

    /// Delete a constraint from the database by their ID.
    ///
    /// Also deletes the constraint from indexing structures if it was added to them.
    #[inline]
    pub fn delete_constraint(
        &mut self,
        context: &mut Context,
        constraint_id: usize,
    ) -> Result<(), CheckingError> {
        match self.entries.get_mut(constraint_id) {
            None => Err(CheckingError::deletion(
                constraint_id,
                &format!(
                    "there are only {} constraints in the database",
                    self.entries.len()
                ),
            )),
            Some(None) => Err(CheckingError::deletion(
                constraint_id,
                "the constraint has already been deleted",
            )),
            Some(entry) => {
                let db_constraint = entry.as_mut().unwrap();
                let was_core = db_constraint.is_core_constraint();
                db_constraint.remove_id(constraint_id);
                if db_constraint.all_constraint_ids_empty(false) {
                    if constraint_id < self.next_unique_index_id {
                        self.unique_constraints.remove(db_constraint.as_ref());
                    }
                    if db_constraint.header.borrow().is_in_occurrences {
                        self.occurrences.remove(db_constraint);
                    }
                    context.propagation_engine.detach(
                        if was_core { CORE } else { DERIVED },
                        db_constraint,
                        true,
                    )?;
                } else if was_core && !db_constraint.is_core_constraint() {
                    move_to_derived_propagator(context, db_constraint)?;
                }
                *entry = None;
                Ok(())
            }
        }
    }

    /// Indexes all constraints that are not yet indexed in `unique_constraints`.
    ///
    /// In case a duplicate constraint is detected, then the duplicate constraint is merged into the first occurrence.
    /// In particular, this also removes duplicate constraints from `occurrences` and the propagation engine.
    ///
    /// Must be called before:
    /// - using `Database::unique_constraints` directly
    /// - calling `Database::lookup()`
    /// - calling `Database::get_proofgoals()`
    pub fn update_unique_index(
        &mut self,
        prop_engine: &mut PropagationEngine,
    ) -> Result<(), CheckingError> {
        for constraint_id in self.next_unique_index_id..self.len() {
            // First check that the constraint ID is not deleted.
            if let Some(Some(constraint)) = self.entries.get(constraint_id) {
                let is_core_constraint = constraint.is_core_constraint();
                if !self.unique_constraints.insert(constraint.clone()) {
                    // Merge this constraint with the existing constraint.
                    let entry = self.unique_constraints.get(constraint).unwrap();
                    let is_original_core_constraint = entry.is_core_constraint();
                    entry.add_id(constraint_id, is_core_constraint);
                    if let Some(out_id) = constraint.get_out_id(constraint_id) {
                        entry.set_out_id(constraint_id, out_id);
                    }
                    constraint.remove_id(constraint_id);
                    // Remove from occurrence list if the duplicate constraint was added.
                    if constraint_id < self.next_occurrences_index_id {
                        self.occurrences.remove(constraint);
                    }
                    // Remove from propagator if it was added.
                    if constraint_id < self.next_propagation_index_id {
                        prop_engine.detach(
                            if is_core_constraint { CORE } else { DERIVED },
                            constraint,
                            true,
                        )?;
                    }
                    // In case the original constraint is only in the derived set
                    // and the new is in the core set, then move it to the core.
                    let is_original_in_propagator = entry.header.borrow().propagator_id.is_some();
                    if is_original_in_propagator
                        && !is_original_core_constraint
                        && is_core_constraint
                    {
                        prop_engine.move_constraint(DERIVED, CORE, entry)?;
                    }
                    // Overwrite the duplicate constraint such that they both IDs point to the same constraint.
                    self.entries[constraint_id] = Some(Rc::clone(entry));
                }
            }
        }
        self.next_unique_index_id = self.len();
        Ok(())
    }

    /// Indexes all constraints that are not yet indexed in `occurrences`. Will be automatically called before occurrences are needed.
    pub fn update_occurrences_index(&mut self) {
        for constraint_id in self.next_occurrences_index_id..self.len() {
            // First check that the constraint ID is not deleted.
            if let Some(Some(constraint)) = self.entries.get(constraint_id) {
                if !constraint.header.borrow().is_in_occurrences {
                    self.occurrences.add(constraint);
                }
            }
        }
        self.next_occurrences_index_id = self.len();
    }

    /// Indexes all constraints that are not yet added to the propagation engine.
    ///
    /// Must be called before:
    /// - calling `PropagationEngine::reverse_unit_propagation_check()`
    /// - calling `PropagationEngine::propagate_solution()`
    pub fn update_propagation_index(
        &mut self,
        prop_engine: &mut PropagationEngine,
    ) -> Result<(), CheckingError> {
        for constraint_id in self.next_propagation_index_id..self.len() {
            // First check that the constraint ID is not deleted.
            if let Some(Some(constraint)) = self.entries.get(constraint_id) {
                if constraint.header.borrow().propagator_id.is_none() {
                    let add_to_core = constraint.is_core_constraint();
                    prop_engine.attach(if add_to_core { CORE } else { DERIVED }, constraint)?;
                }
            }
        }
        self.next_propagation_index_id = self.len();
        Ok(())
    }

    /// Check if `constraint` is contained in the `Database` and return a reference to the constraint in the database if it exists.
    #[inline]
    pub fn lookup(&self, constraint: &Rc<DBConstraint>) -> Option<&Rc<DBConstraint>> {
        debug_assert!(self.len() == self.next_unique_index_id);
        self.unique_constraints.get(constraint)
    }

    /// Check if the database contain a contradiction.
    #[inline]
    pub fn contains_contradiction(&self) -> Option<usize> {
        for constraint in self.unique_constraints.iter() {
            if constraint.is_contradicting() {
                return Some(constraint.get_some_id());
            }
        }
        for constraint_id in self.next_unique_index_id..self.len() {
            if let Some(Some(constraint)) = self.entries.get(constraint_id) {
                if constraint.is_contradicting() {
                    return Some(constraint_id);
                }
            }
        }
        None
    }

    #[inline]
    pub fn normalize_id(&self, index: isize) -> isize {
        if index < 0 {
            self.len() as isize + index
        } else {
            index
        }
    }

    #[inline]
    pub fn get_entry(&self, index: isize) -> Result<&Rc<DBConstraint>, CheckingError> {
        let entry = if index < 0 {
            match self
                .entries
                .get(((self.entries.len() as isize) + index) as usize)
            {
                Some(entry) => entry,
                None => {
                    return Err(CheckingError::out_of_bounds(
                        index,
                        -(self.entries.len() as isize) + 1,
                        (self.entries.len() as isize) - 1,
                    ));
                }
            }
        } else {
            match self.entries.get(index as usize) {
                Some(entry) => entry,
                None => {
                    return Err(CheckingError::out_of_bounds(
                        index,
                        -(self.entries.len() as isize),
                        (self.entries.len() as isize) - 1,
                    ));
                }
            }
        };

        match entry {
            Some(constraint) => Ok(constraint),
            None => Err(CheckingError::access_deleted(index)),
        }
    }

    #[inline]
    pub fn get_entry_usize(&self, index: usize) -> Result<&Rc<DBConstraint>, CheckingError> {
        let entry = match self.entries.get(index) {
            Some(entry) => entry,
            None => {
                return Err(CheckingError::out_of_bounds(
                    index as isize,
                    -(self.entries.len() as isize),
                    (self.entries.len() as isize) - 1,
                ));
            }
        };

        match entry {
            Some(constraint) => Ok(constraint),
            None => Err(CheckingError::access_deleted(index as isize)),
        }
    }

    #[inline]
    pub fn get_entry_optionally_deleted_usize(
        &self,
        index: usize,
    ) -> Result<Option<&Rc<DBConstraint>>, CheckingError> {
        let entry = match self.entries.get(index) {
            Some(entry) => entry,
            None => {
                return Err(CheckingError::out_of_bounds(
                    index as isize,
                    -(self.entries.len() as isize),
                    (self.entries.len() as isize) - 1,
                ));
            }
        };

        match entry {
            Some(constraint) => Ok(Some(constraint)),
            None => Ok(None),
        }
    }

    /// Check if the constraint at the `id` is not deleted.
    #[inline]
    pub fn is_undeleted(&self, index: usize) -> Result<bool, CheckingError> {
        if let Some(entry) = self.entries.get(index) {
            return Ok(entry.is_some());
        }
        Err(CheckingError::out_of_bounds(
            index as isize,
            -(self.entries.len() as isize),
            (self.entries.len() as isize) - 1,
        ))
    }

    /// Move the constraint ID to the core set and add it to the core propagator if it is not already in the core.
    #[inline]
    pub fn move_to_core(
        &self,
        prop_engine: &mut PropagationEngine,
        index: usize,
    ) -> Result<(), CheckingError> {
        // Find the constraint and get its header.
        let constraint = self.get_entry_usize(index)?;

        // If the constraint was not in the core propagator before, it is moved there.
        if !constraint.is_core_constraint() && constraint.header.borrow().propagator_id.is_some() {
            prop_engine.move_constraint(DERIVED, CORE, constraint)?;
        }

        // Remove ID from derived set and add it to the core set of IDs.
        constraint.move_id_to_core(index);

        Ok(())
    }

    /// Move all constraints to the core set and add them to the core propagator.
    pub fn move_to_core_all(
        &self,
        prop_engine: &mut PropagationEngine,
    ) -> Result<(), CheckingError> {
        // Move all constraints in database to core set.
        for constraint in self.unique_constraints.iter() {
            let header = &mut *constraint.header.borrow_mut();
            header.core_ids.append(&mut header.derived_ids);
        }
        for constraint_id in self.next_unique_index_id..self.len() {
            if let Some(Some(constraint)) = self.entries.get(constraint_id) {
                let header = &mut *constraint.header.borrow_mut();
                header.core_ids.append(&mut header.derived_ids);
            }
        }

        // Move all constraints in the derived propagation set to the core propagation set.
        prop_engine.move_all_constraints(DERIVED, CORE)?;

        Ok(())
    }

    /// Get the variables actually used by the constraints in the database.
    #[inline]
    pub fn get_used_vars(&mut self) -> Vec<VarIdx> {
        self.update_occurrences_index();
        self.occurrences.get_used_vars()
    }

    /// Add constraints to `unique_substituted_constraints` that contain the literal `lit`.
    ///
    /// This function only adds constraints that are not obviously implied by the database already.
    #[inline]
    fn add_non_obvious_proofgoals(
        &self,
        substitution: &Substitution,
        unique_touched_constraint: HashSet<&ByAddress<Rc<DBConstraint>>>,
        add_derived_goals: bool,
        exclude_database_autoproving: bool,
        only_core_subproof: bool,
    ) -> HashSet<Rc<DBConstraint>> {
        let mut unique_substituted_constraints =
            HashSet::with_hasher(RandomState::with_seeds(42, 42, 42, 42));
        for constraint in unique_touched_constraint {
            if add_derived_goals || constraint.is_core_constraint() {
                let substituted_constraint = Rc::new(constraint.substitute(substitution));
                // The following conditions have to consider a proofgoal non-obvious:
                // 1. The proofgoal is not trivial (... >= 1)
                // 2. The original constraint does not imply the proofgoal.
                // 3. No other database constraint is syntactially equivalent to the proofgoal. This is ignored, if:
                //     1. `exclude_database_autoproving` is true.
                //     2. We can only use core constraints for the subrpoof and the database constraint is not in the core set.
                if !substituted_constraint.is_trivial()
                    && !constraint.implies(&substituted_constraint)
                    && (exclude_database_autoproving
                        || self
                            .lookup(&substituted_constraint)
                            .is_none_or(|c| only_core_subproof && !c.is_core_constraint()))
                {
                    // Copy ids from the original constraint.
                    substituted_constraint.copy_ids(constraint);
                    // Check if constraint is already contained in unique subsituted constraints.
                    if let Some(prev_entry) =
                        unique_substituted_constraints.replace(substituted_constraint)
                    {
                        let entry = unique_substituted_constraints.get(&prev_entry).unwrap();
                        entry.append_ids(Rc::into_inner(prev_entry).unwrap());
                    }
                }
            }
        }
        unique_substituted_constraints
    }

    /// Get the proofgoals from the formula for strengthening rules with respect to the substitution.
    ///
    /// Proofgoals that are trivial, implied by the original constraint, or that can be found in the database are not added as proofgoals.
    #[inline]
    pub fn get_proofgoals(
        &mut self,
        substitution: &Substitution,
        add_derived_goals: bool,
        exclude_database_autoproving: bool,
        only_core_subproof: bool,
    ) -> HashSet<Rc<DBConstraint>> {
        debug_assert_eq!(self.len(), self.next_unique_index_id);

        self.update_occurrences_index();
        let mut unique_touched_constraints =
            HashSet::with_hasher(RandomState::with_seeds(42, 42, 42, 42));
        for &var_idx in substitution.support.iter() {
            // Constraints where the literal is mapped to true do not need to be considered as a proofgoal.
            match substitution.get(var_idx).unwrap() {
                SubstitutionValue::TRUE => {
                    if let Some(occurrences) = self
                        .occurrences
                        .get_constraints_for_lit(Lit::from_var(var_idx, true))
                    {
                        unique_touched_constraints.extend(occurrences.iter());
                    }
                }
                SubstitutionValue::FALSE => {
                    if let Some(occurrences) = self
                        .occurrences
                        .get_constraints_for_lit(Lit::from_var(var_idx, false))
                    {
                        unique_touched_constraints.extend(occurrences.iter());
                    }
                }
                _ => {
                    if let Some(occurrences) = self
                        .occurrences
                        .get_constraints_for_lit(Lit::from_var(var_idx, false))
                    {
                        unique_touched_constraints.extend(occurrences.iter());
                    }
                    if let Some(occurrences) = self
                        .occurrences
                        .get_constraints_for_lit(Lit::from_var(var_idx, true))
                    {
                        unique_touched_constraints.extend(occurrences.iter());
                    }
                }
            }
        }

        let exclude_database_autoproving = exclude_database_autoproving
            && unique_touched_constraints.len() < self.ignore_database_proofgoal_size;

        self.add_non_obvious_proofgoals(
            substitution,
            unique_touched_constraints,
            add_derived_goals,
            exclude_database_autoproving,
            only_core_subproof,
        )
    }

    #[inline]
    pub fn get_undeleted(
        &self,
        range: Range<usize>,
    ) -> impl '_ + Iterator<Item = Result<usize, CheckingError>> {
        range
            .into_iter()
            .filter_map(|id| match self.is_undeleted(id) {
                Ok(true) => Some(Ok(id)),
                Ok(false) => None,
                Err(err) => Some(Err(err)),
            })
    }

    #[inline]
    pub fn get_undeleted_non_unique_indexed(&self) -> impl '_ + Iterator<Item = &Rc<DBConstraint>> {
        self.entries[self.next_unique_index_id..self.len()]
            .iter()
            .flatten()
    }

    /// The length of the database is the number of entries including deleted constraint and duplicates.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.len() == 0
    }

    /// The size of the database is the number of non-deleted (`Some(constraint)`) entries in the database.
    #[inline]
    pub fn size(&self) -> usize {
        self.entries.iter().flatten().count()
    }
}
