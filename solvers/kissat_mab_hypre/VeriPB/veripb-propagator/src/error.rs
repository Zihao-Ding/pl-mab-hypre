use thiserror::Error;

#[derive(Debug, Error)]
pub enum PropagatorError {
    #[error("This constraint has already been attached to a propagator. The constraint needs to be detached before it can be attached again.")]
    AttachingAttached,

    #[error(
        "This constraint is not been attached. Hence it cannot be detached from a propagator."
    )]
    DetachingUnattached,

    #[error("This constraint has been attached to a different propagator.")]
    AttachedToOtherPropagator,

    #[error("No propagator for this type of constraint in the propagation set!")]
    TypeOfPropagatorNotInSet,

    #[error("The given solution is conflicting with constraint {0} in the database.")]
    SolutionConflictingWithConstraint(usize),

    #[error("The given solution is conflicting with on constraint on the saved trail. Use the option '--trace-failed' to see which constraints propagate which literals.")]
    SolutionConflictingWithSavedTrail,

    #[error("The trail already contains the negated literal")]
    TrailContainsNegatedLit,
}
