//! This file contains useful helper functions that can be used in multiple places for the checker.

use std::rc::Rc;

use veripb_formula::prelude::*;
use veripb_propagator::propagation_engine::PropagationEngine;

use crate::{
    context::{CORE, DERIVED},
    prelude::*,
};

/// Set up a new propagation engine for VeriPB.
#[inline]
pub fn new_veripb_propagation_engine() -> PropagationEngine {
    let mut propagation_engine = PropagationEngine::default();
    propagation_engine.add_set();
    propagation_engine.add_set();
    propagation_engine.add_set();
    propagation_engine.enable_back(CORE);
    propagation_engine.enable_back(DERIVED);
    propagation_engine
}

/// Check the given solution and return it as an assignment.
#[inline]
pub fn check_solution(
    context: &mut Context,
    database: &mut Database,
    solution: &Vec<Lit>,
) -> Result<Assignment<BooleanVar>, CheckingError> {
    let mut assignment = Assignment::from(solution).ok_or(CheckingError::SolutionIsConflicting)?;
    assignment.resize(context.var_names.len());

    database.update_unique_index(&mut context.propagation_engine)?;
    if solution_satisfies_all_constraints(database, &assignment).is_ok_and(|v| v) {
        return Ok(assignment);
    }

    database.update_propagation_index(&mut context.propagation_engine)?;
    let assignment = context.propagation_engine.propagate_solution(
        solution,
        &context.var_names,
        context.args.trace_failed,
    )?;

    solution_satisfies_all_constraints(database, &assignment)?;

    Ok(assignment)
}

/// Check the given complete solution and return it as an assignment.
#[inline]
fn solution_satisfies_all_constraints(
    database: &mut Database,
    assignment: &Assignment<BooleanVar>,
) -> Result<bool, CheckingError> {
    for constraint in database.unique_constraints.iter() {
        if !constraint.is_satisfied(assignment) {
            if constraint.is_falsified(assignment) {
                return Err(CheckingError::SolutionFalsifyingConstraint(
                    constraint.get_some_id(),
                ));
            }
            return Err(CheckingError::SolutionNotSatisfiedConstraint(
                constraint.get_some_id(),
            ));
        }
    }
    Ok(true)
}

/// Do the syntactic implication check with saturation.
///
/// This check can derive a constraint `target` from another constraint `source` by adding literal axioms to it, performing saturation and another round of adding literal axioms.
///
/// Already takes care to check if the `source` constraint is a core constraint in a core only subproof.
#[inline]
pub fn check_implication(
    context: &mut Context,
    database: &mut Database,
    target: &Rc<DBConstraint>,
    hint: Option<isize>,
) -> Result<isize, CheckingError> {
    match hint {
        Some(hint) => {
            let index = database.normalize_id(hint);
            let entry = database.get_entry_usize(index as usize)?;
            if context.only_core && !entry.is_core_constraint_id(index as usize) {
                return Err(CheckingError::CoreSubproofUsingNonCoreConstraint(hint));
            }
            if !entry.implies(target) {
                return Err(CheckingError::not_implied(target, entry));
            }
            Ok(index)
        }
        None => {
            database.update_unique_index(&mut context.propagation_engine)?;
            if let Some(constraint) = database.lookup(target) {
                if !context.only_core || constraint.is_core_constraint() {
                    return Ok(constraint.get_some_id() as isize);
                }
            }

            for constraint in database.unique_constraints.iter() {
                if (!context.only_core || constraint.is_core_constraint())
                    && constraint.implies(target)
                {
                    return Ok(constraint.get_some_id() as isize);
                }
            }

            Err(CheckingError::not_implied_db(target, context.only_core))
        }
    }
}

/// Do the syntactic implication check with saturation for the substituted database.
///
/// This check is stronger than the syntactic implication check, since it substitutes `source` and `target` constraints using the current unit propagation before checking if the `source` implies `target`.
///
/// Already takes care to check if the `source` constraint is a core constraint in a core only subproof.
#[inline]
pub fn check_substituted_implication(
    context: &mut Context,
    database: &mut Database,
    target: &Rc<DBConstraint>,
    hint: Option<isize>,
) -> Result<isize, CheckingError> {
    match hint {
        Some(hint) => {
            let propagated_assignment = context
                .propagation_engine
                .get_propagated_assignment(context.var_names.len(), context.only_core);
            let sub_target = &Rc::new(target.substitute(propagated_assignment));
            let index = database.normalize_id(hint);
            let entry = database.get_entry_usize(index as usize)?;
            let sub_source = &Rc::new(entry.substitute(propagated_assignment));
            if context.only_core && !entry.is_core_constraint_id(index as usize) {
                return Err(CheckingError::CoreSubproofUsingNonCoreConstraint(hint));
            }
            if !sub_source.implies(sub_target) {
                return Err(CheckingError::not_implied_substituted(sub_target, entry));
            }
            Ok(index)
        }
        None => {
            database.update_unique_index(&mut context.propagation_engine)?;
            let propagated_assignment = context
                .propagation_engine
                .get_propagated_assignment(context.var_names.len(), context.only_core);
            let sub_target = &Rc::new(target.substitute(propagated_assignment));
            if let Some(constraint) = database.lookup(target) {
                if !context.only_core || constraint.is_core_constraint() {
                    return Ok(constraint.get_some_id() as isize);
                }
            }
            if let Some(constraint) = database.lookup(sub_target) {
                if !context.only_core || constraint.is_core_constraint() {
                    return Ok(constraint.get_some_id() as isize);
                }
            }

            for constraint in database.unique_constraints.iter() {
                if !context.only_core || constraint.is_core_constraint() {
                    let sub_constraint = constraint.substitute(propagated_assignment);
                    if sub_constraint.implies(sub_target) {
                        return Ok(constraint.get_some_id() as isize);
                    }
                }
            }

            Err(CheckingError::not_implied_substituted_db(
                sub_target,
                context.only_core,
            ))
        }
    }
}
