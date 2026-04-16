//! Collection of [`PBConstraint`](crate::pb_constraint::PBConstraint) with [`PBObjective`] to form a problem.

use crate::{pb_constraint::PBConstraintEnum, pb_objective::PBObjective};

/// Data structure to store a collection of constraints and an optional objective function.
#[derive(Debug, Default)]
pub struct Formula {
    /// The constraints in the problem.
    pub constraints: Vec<PBConstraintEnum>,
    /// The objective to be minimized.
    pub objective: Option<PBObjective>,
}

impl Formula {
    /// Create a new [`Formula`] with an initial allocated capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Formula {
            constraints: Vec::with_capacity(capacity),
            objective: None,
        }
    }

    /// Returns the number of constraints in the [`Formula`].
    #[inline]
    pub fn len(&self) -> usize {
        self.constraints.len()
    }

    /// Test if the [`Formula`] does contain any constraint.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    /// The allocated capacity for the [`Formula`].
    #[inline]
    pub fn capacity(&self) -> usize {
        self.constraints.capacity()
    }

    /// Reserve capacity for exactly `additional` many more constraints in the constraint set.
    ///
    /// This function can be used to avoid reallocation of the memory inside the constraint set when the number of `additional` constraints is known.
    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.constraints.reserve_exact(additional);
    }
}
