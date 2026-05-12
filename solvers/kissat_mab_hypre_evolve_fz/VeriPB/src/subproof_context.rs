use std::{collections::BTreeMap, rc::Rc};

use colored::Colorize;
use veripb_formula::prelude::*;

use crate::{
    context::AUTOPROVING,
    prelude::*,
    rules::{ObjectiveUpdateType, ScopeId},
};

const PROOFGOAL_ELABORATION_THRESHOLD: f64 = 0.05;

#[derive(Debug, Default)]
pub struct SubproofContext {
    // Constraints to be added after the subproof has been completed.
    pub to_add: Vec<Rc<DBConstraint>>,
    // Add the constraints in the `to_add` vector to the core set.
    pub add_to_core: bool,
    // Proofgoals from the database that should be proven in the subproof.
    database_proofgoals: BTreeMap<usize, Proofgoal>,
    // Internal proofgoals that should be proven in the subproof. Internal proofgoals are identified by an `#`.
    internal_proofgoals: Vec<Option<Proofgoal>>,
    // Objective that should be set after the subproof is completed. This can update
    pub objective_update: Option<PBObjective>,
    // The type of the objective update.
    pub objective_update_type: ObjectiveUpdateType,
    // The additional premise is usually the negated constraint.
    pub additional_hint: Option<usize>,
    // Flag to for determining if we are inside a proofgoal subproof of this subproof.
    pub proofgoal_subproof: bool,
    // Flag if proof by contradiction subproof.
    pub proof_by_contradiction_subproof: bool,
    // Start constraint ID of the subproof.
    pub subproof_start_id: usize,
    // Current scope the subproof is in.
    pub current_scope: Option<ScopeId>,
    // Starting constraint ID for the scope.
    pub scope_start_id: usize,
    // Witness used for this subproof.
    pub witness: Substitution,
    // Contradiction proven.
    pub contradiction_proven: bool,
    // Contradiction output hint.
    pub contradiction_output_id: usize,
}

impl SubproofContext {
    /// Creates a new subproof context with internal proofgoals containing dummy 0th element.
    #[inline]
    pub fn new(start_id: usize) -> Self {
        SubproofContext {
            internal_proofgoals: vec![None],
            subproof_start_id: start_id,
            ..Default::default()
        }
    }

    /// Creates a new subproof context for proofgoal subproofs.
    #[inline]
    pub fn new_proofgoal_subproof(start_id: usize) -> Self {
        SubproofContext {
            proofgoal_subproof: true,
            subproof_start_id: start_id,
            ..Default::default()
        }
    }

    /// Add a single constraint internal proofgoal to the subproof context.
    #[inline]
    pub fn add_single_proofgoal(
        &mut self,
        constraint: Rc<DBConstraint>,
        scope_restriction: Option<ScopeId>,
    ) {
        self.internal_proofgoals
            .push(Some(Proofgoal::mk_single_constraint(
                constraint,
                scope_restriction,
                false,
            )));
    }

    /// Add a proofgoal as an internal proofgoal.
    #[inline]
    pub fn add_internal_proofgoal(&mut self, proofgoal: Proofgoal) {
        self.internal_proofgoals.push(Some(proofgoal));
    }

    /// Add dummy internal proofgoals to increment the IDs.
    pub fn resize_internal(&mut self, new_size: usize) {
        self.internal_proofgoals.resize_with(new_size, || None);
    }

    /// Add a multi constraint internal proofgoal to the subproof context.
    #[inline]
    pub fn add_multi_proofgoal(
        &mut self,
        constraints: Vec<Rc<DBConstraint>>,
        scope_restriction: Option<ScopeId>,
    ) {
        self.internal_proofgoals
            .push(Some(Proofgoal::mk_multi_constraint(
                constraints,
                scope_restriction,
            )));
    }

    /// Add a single constraint proofgoal from the database to the subproof context.
    ///
    /// The constraint is added in such a way that it can be easily referenced by the constraint ID of any duplicate of the constraint. However, when the proofgoal is proven, this fact should directly propagate to the other duplicates.
    #[inline]
    pub fn add_database_proofgoal(
        &mut self,
        constraint: Rc<DBConstraint>,
        add_derived_goals: bool,
        scope_restriction: Option<ScopeId>,
    ) {
        for &id in constraint.header.borrow().core_ids.iter() {
            self.database_proofgoals.insert(
                id,
                Proofgoal::mk_single_constraint(constraint.clone(), scope_restriction, false),
            );
        }
        if add_derived_goals {
            for &id in constraint.header.borrow().derived_ids.iter() {
                self.database_proofgoals.insert(
                    id,
                    Proofgoal::mk_single_constraint(constraint.clone(), scope_restriction, false),
                );
            }
        }
    }

    /// Get an internal proof goal.
    #[inline]
    pub fn pop_internal_goal(&mut self, index: usize) -> Result<Proofgoal, CheckingError> {
        if let Some(optional_goal) = self.internal_proofgoals.get_mut(index) {
            if let Some(goal) = optional_goal.take() {
                if let Some(scope) = self.current_scope {
                    if !goal.is_in_scope(scope) {
                        return Err(CheckingError::ProofgoalNotInScope(
                            ProofgoalID::Internal(index),
                            scope,
                        ));
                    }
                }
                return Ok(goal);
            }
            return Err(CheckingError::InternalProofgoalAlreadyProven(index));
        }
        Err(CheckingError::InternalProofgoalNotExisting(index))
    }

    /// Get a database proofgoal.
    #[inline]
    pub fn pop_database_goal(
        &mut self,
        index: usize,
        database: &Database,
    ) -> Result<Proofgoal, CheckingError> {
        if let Some(goal) = self.database_proofgoals.remove(&index) {
            if let Some(scope) = self.current_scope {
                if !goal.is_in_scope(scope) {
                    return Err(CheckingError::ProofgoalNotInScope(
                        ProofgoalID::Database(index as isize),
                        scope,
                    ));
                }
            }
            Ok(goal)
        } else {
            let constraint = database.get_entry_usize(index)?;
            Ok(Proofgoal::mk_single_constraint(
                Rc::new(constraint.substitute(&self.witness)),
                None,
                false,
            ))
        }
    }

    /// Prints the last added internal proofgoal.
    #[inline]
    pub fn trace_internal_proofgoal_back(&self, var_names: &VarNameManager) {
        self.trace_internal_proofgoal(self.internal_proofgoals.len() - 1, var_names);
    }

    /// Prints the internal proofgoal with the given `id`.
    #[inline]
    pub fn trace_internal_proofgoal(&self, id: usize, var_names: &VarNameManager) {
        if let Some(Some(proofgoal)) = self.internal_proofgoals.get(id) {
            let id_string = String::from("#") + id.to_string().as_str();
            proofgoal.trace(id_string.as_str(), var_names)
        }
    }

    /// Print all the internal proofgoals.
    #[inline]
    pub fn trace_all_internal_proofgoals(&self, var_names: &VarNameManager) {
        for (goal_id, goal) in self.internal_proofgoals.iter().enumerate() {
            let id_string = String::from("#") + goal_id.to_string().as_str();
            if let Some(proofgoal) = goal {
                proofgoal.trace(id_string.as_str(), var_names)
            }
        }
    }

    /// Print the proofgoals from the database according to their constraint ID.
    #[inline]
    pub fn trace_database_proofgoals(&self, var_names: &VarNameManager) {
        if self.database_proofgoals.is_empty() {
            return;
        }
        println!("  ** proofgoal from formula **");
        for (goal_id, proofgoal) in self.database_proofgoals.iter() {
            println!(
                "  proofgoal {}: {}",
                goal_id.to_string().bright_green(),
                proofgoal
                    .unwrap_conclusion()
                    .to_pretty_string(var_names)
                    .blue()
            );
        }
    }

    /// Finalizes the subproof by checking for unproven proofgoals and autoproving them if possible.
    ///
    /// This function can be used to conclude a top-level subproof (i.e., not a proofgoal subproof). It tries to autoprove the remaining proofgoals. If all proofgoals are proven, then this function updates the objective and return the constraints to be added after the subproof is successful.
    ///
    /// This function consumes the `SubproofContext`, as it is no longer required and saves reallocating memory for the output constraints that should be added.
    pub fn finalize(
        mut self,
        context: &mut Context,
        database: &mut Database,
        additional_premises: &[Rc<DBConstraint>],
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if context.args.trace {
            println!("  autoproving remaining proofgoals:")
        }

        // Check if all proofgoals have been proven.
        if !self.proof_by_contradiction_subproof
            && self.internal_proofgoals.iter().all(|g| g.is_none())
            && self.database_proofgoals.is_empty()
        {
            if let Some(elaborator) = context.elaborator.as_mut() {
                elaborator.writeln("qed;");
            }
            return self.finalize_unchecked(context);
        } else if self.contradiction_proven {
            if let Some(elaborator) = context.elaborator.as_mut() {
                elaborator.write("qed : ");
                elaborator.write(&self.contradiction_output_id.to_string());
                elaborator.writeln(";");
            }
            return self.finalize_unchecked(context);
        }

        // Set up proof buffer for elaboration.
        let mut proof_buf = None;
        if let Some(elaborator) = context.elaborator.as_mut() {
            proof_buf = Some(&mut elaborator.proof_buf);
        }

        // Check if we can derive contradiction in the top-level subproof. Hence, all proofgoals can use this contradiction to show the proofgoals.
        let hint = if additional_premises.is_empty() {
            if self.has_many_proofgoals() || self.proof_by_contradiction_subproof {
                database.update_propagation_index(&mut context.propagation_engine)?;
                if context
                    .propagation_engine
                    .reverse_unit_propagation_check(
                        &context.var_names,
                        &[],
                        None,
                        context.only_core,
                        &mut proof_buf,
                        self.proof_by_contradiction_subproof && context.args.trace_failed,
                    )?
                    .is_conflict()
                {
                    if context.args.trace {
                        println!(
                            "  * database with subproof constraints is propagating to conflict"
                        );
                    }
                    context.rup_streak += 1;
                    if let Some(elaborator) = context.elaborator.as_mut() {
                        elaborator.write("\trup >= 1 :");
                        elaborator.write_and_clear_buf();
                        elaborator.writeln(";");
                        elaborator.write("qed : ");
                        let next_id = elaborator.inc_id();
                        elaborator.write(&next_id.to_string());
                        elaborator.writeln(";");
                    }
                    return self.finalize_unchecked(context);
                }
            }
            if let Some(premise) = self.additional_hint {
                Some(database.get_entry_usize(premise)?.clone())
            } else {
                None
            }
        } else {
            if self.has_many_proofgoals() || self.proof_by_contradiction_subproof {
                database.update_propagation_index(&mut context.propagation_engine)?;
                if context
                    .propagation_engine
                    .reverse_unit_propagation_check(
                        &context.var_names,
                        additional_premises,
                        None,
                        context.only_core,
                        &mut proof_buf,
                        self.proof_by_contradiction_subproof && context.args.trace_failed,
                    )?
                    .is_conflict()
                {
                    if context.args.trace {
                        println!("  * database with subproof premises is propagating to conflict");
                    }
                    context.rup_streak += 1;
                    if let Some(elaborator) = context.elaborator.as_mut() {
                        elaborator.write("\trup >= 1 :");
                        elaborator.write_and_clear_buf();
                        elaborator.writeln(";");
                        elaborator.write("qed : ");
                        let next_id = elaborator.inc_id();
                        elaborator.write(&next_id.to_string());
                        elaborator.writeln(";");
                    }
                    return self.finalize_unchecked(context);
                }
            }

            // Add additional premises to the propagation engine for the autoproving.
            for constraint in additional_premises {
                context.propagation_engine.attach(AUTOPROVING, constraint)?;
            }
            context.propagation_engine.enable_front(AUTOPROVING);

            // Take the first constraint as hint, as this is the negated constraint for redundance- and dominance-based strengthening.
            Some(additional_premises[0].clone())
        };

        if self.proof_by_contradiction_subproof {
            return Err(CheckingError::ProofByContradictionNotRUP);
        }

        // Check remaining internal proofgoals.
        for (id, proofgoal) in self.internal_proofgoals.iter_mut().enumerate() {
            if let Some(proofgoal) = proofgoal {
                let technique = proofgoal
                    .autoprove(context, database, &hint, ProofgoalID::Internal(id), true)
                    .inspect_err(|_| {
                        if !additional_premises.is_empty() {
                            context.propagation_engine.disable(AUTOPROVING);
                            context.propagation_engine.clear_set(AUTOPROVING);
                        }
                    })?;
                if context.args.trace {
                    proofgoal.trace_autoproven(
                        ("#".to_string() + id.to_string().as_str()).purple(),
                        technique,
                    );
                }
            }
        }

        let elaborate_database_proofgoals = (self.database_proofgoals.len() as f64
            / database.len() as f64)
            <= PROOFGOAL_ELABORATION_THRESHOLD;
        // Check remaining database proofgoals.
        while let Some((id, mut proofgoal)) = self.database_proofgoals.pop_first() {
            let technique = proofgoal
                .autoprove(
                    context,
                    database,
                    &hint,
                    ProofgoalID::Database(id as isize),
                    elaborate_database_proofgoals,
                )
                .inspect_err(|_| {
                    if !additional_premises.is_empty() {
                        context.propagation_engine.disable(AUTOPROVING);
                        context.propagation_engine.clear_set(AUTOPROVING);
                    }
                })?;
            if context.args.trace {
                proofgoal.trace_autoproven(id.to_string().bright_green(), technique);
            }
        }

        if let Some(elaborator) = context.elaborator.as_mut() {
            elaborator.writeln("qed;");
        }

        if !additional_premises.is_empty() {
            context.propagation_engine.disable(AUTOPROVING);
            context.propagation_engine.clear_set(AUTOPROVING);
        }

        self.finalize_unchecked(context)
    }

    /// Finalize the subproof without checking for unproven proofgoals.
    ///
    /// This function updates the objective and return the constraints to be added after the subproof is successful.
    #[inline]
    pub fn finalize_unchecked(
        self,
        context: &mut Context,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        // Change the objective if it should be updated.
        if let Some(objective_update) = self.objective_update {
            context.update_objective(objective_update, self.objective_update_type);
        }

        // Add the derived constraints.
        Ok(self.to_add)
    }

    /// The maximum ID for an internal proofgoal.
    #[inline]
    pub fn internal_proofgoal_len(&self) -> usize {
        self.internal_proofgoals.len()
    }

    /// The number of database proofgoals.
    #[inline]
    pub fn database_proofgoals_len(&self) -> usize {
        self.database_proofgoals.len()
    }

    /// Check if there are many proofgoals.
    ///
    /// This is used for heuristic reasons to detect if we should use the top-level RUP check to detect if all proofgoals follow by one RUP check without using the proofgoals at all. Usually this is the case for redundance-based strengthening steps that could be RUP steps.
    ///
    /// As a lower bound this function should return true if there is at least one internal proofgoal (which are offset by one) and at least one database proofgoals.
    #[inline]
    fn has_many_proofgoals(&self) -> bool {
        self.internal_proofgoal_len() >= 2 && self.database_proofgoals.len() >= 3
    }
}
