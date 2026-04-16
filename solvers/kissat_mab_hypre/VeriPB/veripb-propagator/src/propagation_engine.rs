use std::{collections::VecDeque, rc::Rc};

use colored::Colorize;
use veripb_formula::prelude::*;

use crate::{
    error::PropagatorError,
    propagation_set::PropagationSet,
    propagator::PropagationResult,
    trail::{Propagation, Reason, Trail},
};

const DERIVED: usize = 1;

#[derive(Debug, Default)]
struct TrailLevel {
    level: usize,
    conflict: bool,
}

/// The `PropagationEngine` keeps track of the propagation sets known to a tool.
///
/// This data structure maintains the propagation sets and provides helpful functions for checks involving unit propagation.
#[derive(Debug, Default)]
pub struct PropagationEngine {
    trail: Trail,
    propagation_sets: Vec<Option<PropagationSet>>,
    temp_propagation_set: PropagationSet,
    /// The order in which to propagate the propagation sets of the propagator.
    /// If the constraint set is not in the order, then the propagation set is deactivated.
    propagation_order: VecDeque<usize>,
    /// If `added` is `true`, then a constraint was added to the propagator and the saved trail might be reconstructed.
    added: bool,
    /// If `true`, then the current saved trail is only with respect to core constraints.
    pub only_core_trail: bool,
    /// The previously saved level.
    saved_level: TrailLevel,
    /// If `true`, then a saved reason has been detached since last propagation.
    saved_reason_detached: bool,
}

impl PropagationEngine {
    /// Create a new propagation set inside the propagation engine.
    #[inline]
    pub fn add_set(&mut self) -> usize {
        self.propagation_sets.push(Some(PropagationSet::default()));
        self.propagation_sets.len() - 1
    }

    /// Remove a propagation set from the propagation engine.
    #[inline]
    pub fn remove_set(&mut self, idx: usize) {
        self.propagation_sets[idx] = None;
    }

    /// Clears the specified propagation set.
    #[inline]
    pub fn clear_set(&mut self, idx: usize) {
        self.propagation_sets[idx].as_mut().unwrap().clear();
    }

    /// The number of propagation sets known to the propagation engine. This number does **not** include deleted propagation sets.
    #[inline]
    pub fn number_of_sets(&self) -> usize {
        self.propagation_sets
            .iter()
            .filter(|set| set.is_some())
            .count()
    }

    /// Attach a constraint to a specific propagation set.
    #[inline]
    pub fn attach(
        &mut self,
        idx: usize,
        constraint: &Rc<DBConstraint>,
    ) -> Result<(), PropagatorError> {
        self.propagation_sets[idx]
            .as_mut()
            .unwrap()
            .add(constraint)?;
        self.added = true;
        Ok(())
    }

    /// Remove a constraint from a specific propagation set.
    #[inline]
    pub fn detach(
        &mut self,
        idx: usize,
        constraint: &Rc<DBConstraint>,
        poison: bool,
    ) -> Result<(), PropagatorError> {
        if poison && constraint.header.borrow().is_saved_reason {
            self.saved_reason_detached = true;
        }
        self.propagation_sets[idx]
            .as_mut()
            .unwrap()
            .remove(constraint)?;
        Ok(())
    }

    /// Move single constraint from one propagator to another propagator that is active.
    #[inline]
    pub fn move_constraint(
        &mut self,
        from: usize,
        to: usize,
        constraint: &Rc<DBConstraint>,
    ) -> Result<(), PropagatorError> {
        debug_assert!(self.is_enabled(to));
        self.detach(from, constraint, false)?;
        self.attach(to, constraint)?;
        Ok(())
    }

    /// Move all constraints from one propagator to another propagator that is active.
    #[inline]
    pub fn move_all_constraints(&mut self, from: usize, to: usize) -> Result<(), PropagatorError> {
        debug_assert!(self.is_enabled(to));
        for constraint in self.propagation_sets[from]
            .as_mut()
            .unwrap()
            .remove_all()
            .iter()
        {
            self.propagation_sets[to]
                .as_mut()
                .unwrap()
                .add(constraint)?;
        }
        self.added = true;
        Ok(())
    }

    /// Check if the propagation set is enabled.
    #[inline]
    pub fn is_enabled(&self, idx: usize) -> bool {
        self.propagation_order.contains(&idx)
    }

    /// Get the position of the propagation set in the propagation order.
    ///
    /// If the propagator is not enabled, then `None` is returned.
    #[inline]
    pub fn get_position(&self, idx: usize) -> Option<usize> {
        self.propagation_order.iter().position(|&i| i == idx)
    }

    /// Enable the propagation set and add it to the start of the propagation order.
    ///
    /// If the propagation set is already enabled in the propagation order, then the propagation set is moved to the front.
    #[inline]
    pub fn enable_front(&mut self, idx: usize) {
        self.disable(idx);
        self.propagation_order.push_front(idx);
    }

    /// Enable the propagation set and add it to the back of the propagation order.
    ///
    /// If the propagation set is already enabled in the propagation order, then the propagation set is moved to the back.
    #[inline]
    pub fn enable_back(&mut self, idx: usize) {
        self.disable(idx);
        self.propagation_order.push_back(idx);
    }

    /// Remove the propagation set from the propagation order.
    ///
    /// This disables this propagation set for the engine. This does **not** delete the propagation set from the propagation engine so that it can be reenabled later on.
    #[inline]
    pub fn disable(&mut self, idx: usize) {
        // Remove from propagation order if it is enabled.
        if let Some(pos) = self.get_position(idx) {
            if self.propagation_sets[idx]
                .as_ref()
                .expect("Propagation set does not exist!")
                .has_saved_reason()
            {
                self.saved_reason_detached = true;
            }
            self.propagation_order.remove(pos);
        }
    }

    /// Propagate the enabled propagation sets in order until a fix point is reached.
    ///
    /// This function propagates the enabled propagation sets in the specified propagation order.
    pub fn propagate(&mut self, using_temp: bool, mark_reasons: bool) -> PropagationResult {
        let mut prev_trail_length = self.trail.len();
        loop {
            if using_temp
                && self
                    .temp_propagation_set
                    .propagate(&mut self.trail, false)
                    .is_conflict()
            {
                return PropagationResult::Conflict;
            }
            for &idx in self.propagation_order.iter() {
                if self.propagation_sets[idx]
                    .as_mut()
                    .unwrap()
                    .propagate(&mut self.trail, mark_reasons)
                    .is_conflict()
                {
                    return PropagationResult::Conflict;
                }
            }
            if prev_trail_length == self.trail.len() {
                break;
            }
            prev_trail_length = self.trail.len();
        }

        PropagationResult::Unknown
    }

    pub fn get_assignment_independent_propagations(
        &mut self,
        using_temp: bool,
    ) -> Vec<Propagation> {
        let mut propagations = Vec::new();

        if using_temp {
            propagations.append(
                &mut self
                    .temp_propagation_set
                    .get_assignment_independent_propagations(),
            );
        }

        for &idx in self.propagation_order.iter() {
            propagations.append(
                &mut self.propagation_sets[idx]
                    .as_mut()
                    .unwrap()
                    .get_assignment_independent_propagations(),
            );
        }

        propagations
    }

    pub fn get_new_assignment_independent_propagations(&mut self) -> Vec<Propagation> {
        let mut new_propagations = Vec::new();

        for &idx in self.propagation_order.iter() {
            self.propagation_sets[idx]
                .as_mut()
                .unwrap()
                .get_new_assignment_independent_propagations(&mut new_propagations);
        }

        new_propagations
    }

    pub fn reset_active_sets(&mut self, prev_trail_len: usize) {
        for &idx in self.propagation_order.iter() {
            self.propagation_sets[idx]
                .as_mut()
                .unwrap()
                .reset(prev_trail_len)
        }
    }

    pub fn resize_sets(&mut self, num_variables: usize) {
        self.temp_propagation_set.resize(num_variables);
        for set in self.propagation_sets.iter_mut().flatten() {
            set.resize(num_variables);
        }
    }

    fn temporarily_add_negated_constraint(
        &mut self,
        constraint: &Rc<DBConstraint>,
        proof_buf: &mut Option<&mut String>,
    ) -> Result<(), PropagatorError> {
        // Negate the desired constraint.
        let negated_constraint = Rc::new(DBConstraint::from(constraint.constraint.negate()));
        if proof_buf.is_some() {
            // 0 is a special ID used for the negated constraint
            negated_constraint.set_out_id(0, 0);
        }
        match self.temp_propagation_set.add(&negated_constraint) {
            Ok(_) | Err(PropagatorError::AttachingAttached) => Ok(()),
            Err(error) => Err(error),
        }
    }

    #[inline]
    fn init_trail(
        &mut self,
        num_vars: usize,
        only_core: bool,
        proof_buf: &mut Option<&mut String>,
    ) -> PropagationResult {
        self.resize_sets(num_vars);
        self.trail.resize(num_vars);

        let mut derived_just_enabled = false;
        match (self.only_core_trail, only_core) {
            (false, true) => {
                // Change from core+derived propagation to only core propagation.
                self.only_core_trail = true;
                self.disable(DERIVED);
            }
            (true, false) => {
                // Change from only core propagation to core+derived propagation.
                self.only_core_trail = false;
                self.enable_back(DERIVED);
                derived_just_enabled = true;
            }
            _ => {}
        }
        let prev_trail_len = self.trail.len();

        let mut propagate = if self.saved_reason_detached {
            if self.unpoison(only_core) {
                self.reset_active_sets(0);

                // WARNING: if rewriting this, careful with the short-circuiting semantics!
                if self.trail.is_conflicting()
                    || self
                        .get_assignment_independent_propagations(false)
                        .into_iter()
                        .any(|propagation| self.trail.push(propagation, true).is_err())
                {
                    self.analyze(proof_buf);
                    return PropagationResult::Conflict;
                }

                self.added = false;

                true
            } else {
                self.reset_active_sets(prev_trail_len);
                false
            }
        } else {
            self.reset_active_sets(prev_trail_len);
            false
        };
        if derived_just_enabled {
            // Add propagations that may have been removed after core only propagation.
            if self.propagation_sets[DERIVED]
                .as_mut()
                .unwrap()
                .get_assignment_independent_propagations()
                .into_iter()
                .any(|propagation| self.trail.push(propagation, true).is_err())
            {
                self.analyze(proof_buf);
                return PropagationResult::Conflict;
            }
        }
        if self.added {
            self.added = false;
            // Get level zero propagations of newly added constraints.
            if self
                .get_new_assignment_independent_propagations()
                .into_iter()
                .any(|propagation| self.trail.push(propagation, true).is_err())
            {
                self.analyze(proof_buf);
                return PropagationResult::Conflict;
            }

            propagate = true;
        };

        // WARNING: if rewriting this, careful with the short-circuiting semantics!
        let conflict =
            self.trail.is_conflicting() || (propagate && self.propagate(false, true).is_conflict());

        if conflict {
            self.analyze(proof_buf);
            return PropagationResult::Conflict;
        }

        self.save_level();

        PropagationResult::Unknown
    }

    pub fn reverse_unit_propagation_check(
        &mut self,
        var_names: &VarNameManager,
        assumptions: &[Rc<DBConstraint>],
        conclusion: Option<&Rc<DBConstraint>>,
        only_core: bool,
        proof_buf: &mut Option<&mut String>,
        trace_failed: bool,
    ) -> Result<PropagationResult, PropagatorError> {
        // Check if conclusion is trivial.
        if let Some(constraint) = conclusion {
            if constraint.is_trivial() {
                if let Some(proof_buf) = proof_buf {
                    proof_buf.push_str(" ~ ");
                }
                return Ok(PropagationResult::Conflict);
            }
        }

        // Reset the trail without considering the conclusion.
        if self
            .init_trail(var_names.len(), only_core, proof_buf)
            .is_conflict()
        {
            return Ok(PropagationResult::Conflict);
        };

        assumptions.iter().try_for_each(|constraint| {
            match self.temp_propagation_set.add(constraint) {
                Ok(_) | Err(PropagatorError::AttachingAttached) => Ok(()),
                Err(err) => Err(err),
            }
        })?;

        if let Some(c) = conclusion {
            self.temporarily_add_negated_constraint(c, proof_buf)?;
            self.temp_propagation_set.reset(self.trail.len());
        }

        let conflict = {
            // WARNING: if rewriting this, careful with the short-circuiting semantics!
            let zero_conflict = self
                .temp_propagation_set
                .get_assignment_independent_propagations()
                .into_iter()
                .any(|propagation| self.trail.push(propagation, false).is_err());
            zero_conflict || self.propagate(true, false).is_conflict()
        };

        let result = if conflict {
            self.analyze(proof_buf);
            PropagationResult::Conflict
        } else {
            if trace_failed {
                self.trace_failed(var_names);
            }
            PropagationResult::Unknown
        };

        self.temp_propagation_set.clear();
        // Restore trail to previous level.
        self.load_level();

        Ok(result)
    }

    /// Analyze the propagation trail to generate hints for reverse unit propagation.
    ///
    /// This function expects that the `trail` is contradicting.
    #[inline]
    fn analyze(&self, proof_buf: &mut Option<&mut String>) {
        if proof_buf.is_none() {
            return;
        }
        let proof_buf = proof_buf.as_mut().unwrap();
        let mut output_ids = Vec::new();
        let mut relevant_assignment: Assignment<BooleanVar> =
            Assignment::with_size(self.trail.assignment.len());

        // Analyze the conflicting constraint.
        let conflict_constraint = self.trail.conflict.as_ref().unwrap().unwrap();
        let mut prev_constraint = conflict_constraint;
        output_ids.push(
            conflict_constraint
                .get_out_id(conflict_constraint.get_some_id())
                .unwrap(),
        );
        conflict_constraint
            .constraint
            .mark_negated_lits(&self.trail.assignment, &mut relevant_assignment);

        for propagation in self.trail.trail.iter().rev() {
            // Check if constraint did propagate something relevant.
            if unsafe { relevant_assignment.get_lit_value_unchecked(propagation.lit) }
                == BoolValue::Assigned(true)
            {
                let constraint = propagation.reason.unwrap();
                // Only add propagation if the constraint did not propagate right before.
                if !Rc::ptr_eq(constraint, prev_constraint) {
                    prev_constraint = constraint;
                    output_ids.push(constraint.get_out_id(constraint.get_some_id()).unwrap());

                    // Add literals that were relevant for the propagation of the constraint.
                    constraint
                        .constraint
                        .mark_negated_lits(&self.trail.assignment, &mut relevant_assignment);
                }
            }
        }

        // Go through the output constraint IDs in reverse order and replace ID 0 with `~`.
        for &id in output_ids.iter().rev() {
            if id == 0 {
                proof_buf.push_str(" ~");
            } else {
                proof_buf.push(' ');
                proof_buf.push_str(&id.to_string());
            }
        }
    }

    fn trace_failed(&self, var_names: &VarNameManager) {
        println!("Propagation check failed! The propagation had the following trail:");
        println!("  propagations in format: <assignment> (<reason constraint>)");

        for Propagation { lit, reason } in self.trail.trail.iter() {
            println!(
                "    {} ({})",
                lit.to_pretty_string(var_names).purple(),
                match reason {
                    Reason::Assumption => "by assumption".cyan(),
                    Reason::Constraint(constraint) => constraint.to_pretty_string(var_names).cyan(),
                }
            )
        }

        if let Some(conflict) = &self.trail.conflict {
            println!(
                "    {} ({})",
                "*conflict*".red(),
                match conflict {
                    Reason::Assumption => "by assumption".cyan(),
                    Reason::Constraint(constraint) => constraint.to_pretty_string(var_names).cyan(),
                }
            )
        }
    }

    /// Propagate a (partial) assignment to possibly get more variables assigned.
    ///
    /// This function takes a `Vec<Lit>` as input for the assignment to start building the propagation trail in order of the solution given by the user.
    #[inline]
    pub fn propagate_solution(
        &mut self,
        literals: &Vec<Lit>,
        var_names: &VarNameManager,
        trace_failed: bool,
    ) -> Result<Assignment<BooleanVar>, PropagatorError> {
        // Reset the trail without considering the assignment.
        if self
            .init_trail(var_names.len(), false, &mut None)
            .is_conflict()
        {
            return Err(PropagatorError::SolutionConflictingWithConstraint(
                self.trail.conflict.as_ref().unwrap().unwrap().get_some_id(),
            ));
        };

        // Initialize the trail to the assignment.
        if self.trail.add_assumptions(literals).is_err() {
            if trace_failed {
                self.trace_failed(var_names);
            }
            return Err(PropagatorError::SolutionConflictingWithSavedTrail);
        }

        // Run the actual propagation, i.e., going through the trail and looking at watch list to see which constraints to consider.
        if self.propagate(false, false).is_conflict() {
            if trace_failed {
                self.trace_failed(var_names);
            }
            return Err(PropagatorError::SolutionConflictingWithConstraint(
                self.trail.conflict.as_ref().unwrap().unwrap().get_some_id(),
            ));
        }

        let full_assignment = self.trail.assignment.clone();
        self.load_level();

        Ok(full_assignment)
    }

    #[inline]
    pub fn get_propagated_assignment(
        &mut self,
        num_vars: usize,
        only_core: bool,
    ) -> &Assignment<BooleanVar> {
        self.init_trail(num_vars, only_core, &mut None);
        &self.trail.assignment
    }

    /// This function should only be called after [`get_propagated_assignment()`] has been called and is used similar to analyze to get the unit propagations.
    #[inline]
    pub fn get_propagated_assignment_hints(&self, proof_buf: &mut String) {
        let mut output_ids = Vec::new();

        // Set up previous constraint.
        let mut prev_constraint = None;

        for propagation in self.trail.trail.iter().rev() {
            // Check if constraint did propagate something relevant.
            let constraint = propagation.reason.unwrap();
            // Only add propagation if the constraint did not propagate right before.
            if prev_constraint.is_none_or(|prev| !Rc::ptr_eq(constraint, prev)) {
                prev_constraint = Some(constraint);
                output_ids.push(constraint.get_out_id(constraint.get_some_id()).unwrap());
            }
        }

        // Go through the output constraint IDs in reverse order and replace ID 0 with `~`.
        for &id in output_ids.iter().rev() {
            proof_buf.push(' ');
            proof_buf.push_str(&id.to_string());
        }
    }

    #[inline]
    pub fn get_trail(&self) -> &Trail {
        &self.trail
    }

    #[inline]
    fn unpoison(&mut self, only_core: bool) -> bool {
        debug_assert!(self.saved_reason_detached);
        self.saved_reason_detached = false;
        self.reset_to_valid(only_core)
    }

    #[inline]
    fn reset_last_pos(&mut self) {
        for &idx in self.propagation_order.iter() {
            self.propagation_sets[idx]
                .as_mut()
                .unwrap()
                .reset_last_pos();
        }
    }

    #[inline]
    fn increase_slack(&mut self, lit: Lit) {
        for &idx in self.propagation_order.iter() {
            self.propagation_sets[idx]
                .as_mut()
                .unwrap()
                .increase_slack(lit);
        }
    }

    #[inline]
    fn save_level(&mut self) {
        self.saved_level.level = self.trail.trail.len();
        self.saved_level.conflict = self.trail.conflict.is_some();
    }

    #[inline]
    fn load_level(&mut self) {
        if !self.saved_level.conflict {
            self.trail.conflict = None;
        }
        self.reset_last_pos();
        while self.trail.len() > self.saved_level.level {
            let lit = self.trail.pop().unwrap().lit;
            self.increase_slack(lit);
        }
    }

    #[inline]
    fn reset_to_level(&mut self, level: usize) {
        if self.trail.len() > level {
            self.trail.conflict = None;
            self.reset_last_pos();
        }

        while self.trail.len() > level {
            let lit = self.trail.pop().unwrap().lit;
            self.increase_slack(lit);
        }
    }

    /// Reset the trail so that it only contains constraints that have not been deleted.
    ///
    /// Return `true` iff the resetting changed the trail.
    #[inline]
    fn reset_to_valid(&mut self, only_core: bool) -> bool {
        let mut modified_trail = false;
        let mut level = 0;
        for Propagation { reason, .. } in self.trail.trail.iter() {
            if let Reason::Constraint(constraint) = reason {
                if constraint.all_constraint_ids_empty(only_core) {
                    break;
                }
            } else {
                break;
            }
            level += 1;
        }
        if level < self.trail.len() {
            modified_trail = true
        }

        if let Some(Reason::Constraint(conflict)) = &self.trail.conflict {
            if conflict.all_constraint_ids_empty(only_core) {
                self.trail.conflict = None;
                modified_trail = true;
            }
        }

        self.reset_to_level(level);

        modified_trail
    }
}
