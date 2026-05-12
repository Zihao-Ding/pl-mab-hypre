use std::{num::Saturating, rc::Rc};

use ahash::AHashMap;
use colored::Colorize;
use malachite_bigint::BigInt;
use num_traits::Zero;
use veripb_formula::prelude::*;
use veripb_propagator::propagation_engine::PropagationEngine;

use crate::{
    args::Args,
    order_context::{OrderContext, ReflexivityContext, SpecificationContext, TransitivityContext},
    prelude::*,
    rules::ObjectiveUpdateType,
};

// Index of propagation sets.
pub const CORE: usize = 0;
pub const DERIVED: usize = 1;
pub const AUTOPROVING: usize = 2;

// Constants for the checker.
pub const REQUIRED_RUP_STREAK: Saturating<u8> = Saturating(5);

#[derive(Debug)]
pub enum Subcontext {
    Subproof(SubproofContext),
    Order(OrderContext),
    Transitivity(TransitivityContext),
    Reflexivity(ReflexivityContext),
    Specification(SpecificationContext),
}

impl Subcontext {
    #[inline]
    pub fn is_subproof(&self) -> bool {
        matches!(self, Subcontext::Subproof(_))
    }
}

/// Context to store the state of the verifier.
///
/// This can be thought of as the configuration maintained by the checker.
#[derive(Debug, Default)]
pub struct Context {
    /// Command line arguments
    pub args: Args,
    /// Storage of variable names and used variables.
    pub var_names: VarNameManager,
    /// Original formula.
    pub original_constraints: Vec<Rc<DBConstraint>>,
    /// Original objective.
    pub original_objective: Option<PBObjective>,
    /// Current objective.
    pub objective: Option<PBObjective>,
    /// The currently best objective value logged while checked deletion was enabled.
    pub best_valid_objective_value: Option<BigInt>,
    /// The currently best objective value over all.
    pub best_objective_value: Option<BigInt>,
    /// Stack of currently used subproof contexts.
    pub subcontexts: Vec<Subcontext>,
    /// A flag to determine if we are inside a strengthening subproof, where some rules are not allowed.
    pub inside_strengthening_subproof: bool,
    /// Propagation engine for checks by propagation.
    pub propagation_engine: PropagationEngine,
    /// All orders known to checker.
    pub orders: AHashMap<String, Order>,
    /// The currently active order.
    pub active_order: Option<ActiveOrder>,
    /// Current streak autoproving red rules by the constraint being RUP.
    pub rup_streak: Saturating<u8>,
    /// Stored assignment for annotated RUP steps.
    pub annotated_rup_assignment: Assignment<BooleanVar>,
    /// Proof major version.
    pub major_version: Option<u8>,
    /// Proof minor version.
    pub minor_version: Option<u8>,
    /// Proof flags.
    pub has_output: bool,
    pub has_conclusion: bool,
    pub has_end_proof: bool,
    pub assumption_used: bool,
    pub verification_result: Option<String>,
    /// Elaborator
    pub elaborator: Option<Elaborator>,
    /// The current level that is set
    pub current_level: Option<usize>,
    /// The storage of constraint IDs for each level
    pub level_ids: Vec<Vec<usize>>,
    /// Store if only core constraints can be used for proof.
    pub only_core: bool,
    /// Stores if the strengthening to core is more is enabled.
    pub is_strengthening_to_core: bool,
}

impl Context {
    /// Create a new [`Context`] with the command line arguments `args` and the mapping of labels to IDs in `var_names`.
    pub fn new(args: Args, var_names: VarNameManager) -> Context {
        Context {
            args,
            var_names,
            propagation_engine: new_veripb_propagation_engine(),
            rup_streak: REQUIRED_RUP_STREAK - Saturating(1),
            ..Default::default()
        }
    }

    /// Update the objective to a new objective.
    ///
    /// The `update_type` specifies how the objective should be updated and `objective_update` contains the data of the objective updaet.
    pub fn update_objective(
        &mut self,
        mut objective_update: PBObjective,
        update_type: ObjectiveUpdateType,
    ) {
        match update_type {
            ObjectiveUpdateType::New => self.objective = Some(objective_update),
            ObjectiveUpdateType::Diff => {
                // Change the objective by the diff.
                let objective = self.objective.as_mut().unwrap();
                objective.constant += objective_update.constant;
                while let Some((var, term)) = objective_update.terms.pop_first() {
                    if let Some(existing_term) = objective.terms.get_mut(&var) {
                        objective.constant += existing_term.add_with(term);
                        if existing_term.coeff.is_zero() {
                            objective.terms.remove(&var);
                        }
                    } else {
                        objective.terms.insert(var, term);
                    }
                }
            }
        }

        // Trace objective change.
        if self.args.trace {
            println!(
                "  {} updated to: {}",
                "Objective".bright_green(),
                self.objective
                    .as_ref()
                    .unwrap()
                    .to_pretty_string(&self.var_names)
                    .green()
            );
        }
    }
}
