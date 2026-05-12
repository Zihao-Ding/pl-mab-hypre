use std::{fmt::Debug, rc::Rc};

use veripb_formula::{
    lit::Lit,
    prelude::{Assignment, BooleanVar, DBConstraint},
};

use crate::trail::Propagation;

#[derive(Debug, PartialEq, Eq)]
pub enum WatchInit {
    Conflict,
    Watching {
        propagated: Vec<Lit>,
        watches: Vec<Lit>,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum WatchUpdate {
    Conflict,
    Unknown {
        propagated: Vec<Lit>,
        new_watches: Vec<Lit>,
    },
    UnknownSingle {
        propagated: Option<Lit>,
        new_watches: Lit,
    },
    None,
}

impl WatchUpdate {
    /// Check if the `PropagationResult` is a conflict or not.
    #[inline]
    pub fn is_conflict(&self) -> bool {
        matches!(*self, WatchUpdate::Conflict)
    }
}

#[derive(Debug, PartialEq)]
pub enum LevelZeroPropagationResult {
    Complete(Vec<Propagation>),
    Partial(Vec<Propagation>),
    None,
}

impl LevelZeroPropagationResult {
    #[inline]
    pub fn unwrap(&self) -> &Vec<Propagation> {
        match self {
            Self::Complete(propagations) | Self::Partial(propagations) => propagations,
            Self::None => panic!(),
        }
    }
}

/// It should be assumed that we only watch a constraint, i.e., add watches to the propagator for this constraint if the constraint does not completely propagate or is conflicting in itself.
pub trait Watcher: Debug {
    /// Create watcher from `DBConstraint`.
    fn from_db_constraint(constraint: &Rc<DBConstraint>) -> Self;

    /// Check for propagation at decision level 0.
    ///
    /// E.g., if the constraint is a unit clause then the literal is propagated to true and the function reports that the constraint has fully propagated.
    fn get_assignment_independent_propagations(&self) -> LevelZeroPropagationResult;

    /// Check if can even propagate and thus requires watches.
    ///
    /// If all coefficients are larger than the slack with respect to the exmpty assignment, then this constraint is either conflicting or propagating all its literals independent of the assignment. Also if the right-hand side is non-positive, then the constraint is always satisfied. In both cases computing the propagations with respect to a partial assignment does not change the propagation.
    fn no_watches_required(&mut self) -> bool;

    /// Check if a constraint is propagating at level 0.
    ///
    /// If a constraint is propagating partially or full at level 0, then return true, else if nor propagation happens, then return false.
    fn is_propagating_independent_of_assignment(&self) -> bool;

    /// Initialize the watches of a constraint to be in sync with the current assignment.
    fn init_watches(&mut self, assign: &Assignment<BooleanVar>) -> WatchInit;

    /// Update the watched literal after the literal `old_watch` has been propagated.
    fn update_watch(&mut self, old_watch: Lit, assign: &Assignment<BooleanVar>) -> WatchUpdate;

    /// Get the currently watched literals.
    fn get_watches(&self) -> impl Iterator<Item = &Lit>;

    /// Get the constraint that the watcher is watching.
    fn get_constraint(&self) -> &Rc<DBConstraint>;

    /// Reset the last position to look for a watch.
    fn reset_last_pos(&mut self) {
        unreachable!();
    }

    /// Reset the watches for the constraints.
    fn increase_slack(&mut self, _lit: Lit) {
        unreachable!();
    }
}
