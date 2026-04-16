use std::{path::PathBuf, rc::Rc};

use colored::Colorize;
use fragile::Sticky;
use malachite_bigint::BigInt;
use thiserror::Error;
use veripb_formula::prelude::DBConstraint;
use veripb_parser::error::ParserError;
use veripb_propagator::error::PropagatorError;

use crate::{
    parser::error::ParseError,
    prelude::{ElaborationError, ProofgoalID},
    rules::ScopeId,
};

#[derive(Debug, Error)]
pub enum VeriPBError {
    #[error("Elaboration error: {0}")]
    Elaborator(#[from] ElaborationError),

    #[error("Syntax error while parsing proof file!")]
    Parse(#[from] ParseError),

    #[error("Verification error at {}:{}\n\nCaused by:\n\t{}", file.to_string_lossy(), line, error.to_string().red())]
    Checking {
        error: CheckingError,
        file: PathBuf,
        line: usize,
    },
}

#[derive(Debug, Error)]
pub enum CheckingError {
    #[error("Syntax error while parsing formula file")]
    FomrulaParser(#[from] ParserError),

    #[error("Propagation engine error: {0}")]
    Propagator(#[from] PropagatorError),

    #[error("The compute function for the rule {rule} has not been implemented, yet!")]
    NotImplemented { rule: String },

    #[error(
        "Unable to delete the constraint with ID {} from the database, as {}.", .constraint_id.to_string(), .reason
    )]
    FailedDeletion {
        constraint_id: usize,
        reason: String,
    },

    #[error("Constraint not found in{which_database} database.")]
    NotFoundInDB {
        constraint: Sticky<Rc<DBConstraint>>,
        which_database: String,
    },

    #[error("Expected constraint is not equal to the constraint at the hint.")]
    NotEqual {
        expected_constraint: Sticky<Rc<DBConstraint>>,
        hint_constraint: Sticky<Rc<DBConstraint>>,
    },

    #[error(
        "Constraint not syntactically implied by any constraint in the{which_database} database."
    )]
    NotImpliedDB {
        constraint: Sticky<Rc<DBConstraint>>,
        which_database: String,
    },

    #[error("Expected constraint is not syntactically implied by the constraint at the hint.")]
    NotImplied {
        expected_constraint: Sticky<Rc<DBConstraint>>,
        hint_constraint: Sticky<Rc<DBConstraint>>,
    },

    #[error("Constraint substituted with current unit propagations is not syntactically implied by any constraint in the{which_database} database substituted with current unit propagations.")]
    NotImpliedDBSubstituted {
        constraint: Sticky<Rc<DBConstraint>>,
        which_database: String,
    },

    #[error("Expected constraint substituted with current unit propagations is not syntactically implied by the constraint at the hint substituted with current unit propagations.")]
    NotImpliedSubstituted {
        expected_constraint: Sticky<Rc<DBConstraint>>,
        hint_constraint: Sticky<Rc<DBConstraint>>,
    },

    #[error("Accessing the database out of bound with index {}. The index should be between {} and {}.", index.to_string(), lower, upper)]
    OutOfBoundsAccess {
        index: isize,
        lower: isize,
        upper: isize,
    },

    #[error("Trying to access constraint with ID {} that has already been deleted.", index.to_string())]
    AccessingDeleted { index: isize },

    #[error("This rule requires checked deletion to be activate.")]
    ExpectedCheckDeletion,

    #[error(
        "The formula contains {formula_size} constraints, but the rule expected that there are {expected_size} constraints."
    )]
    FormulaSizeMismatch {
        formula_size: usize,
        expected_size: usize,
    },

    #[error("The output section has already been given.")]
    DoubleOutput,

    #[error("The conclusion section has already been given.")]
    DoubleConclusion,

    #[error("The 'end pseudo-Boolean proof' has already been given.")]
    DoubleEnd,

    #[error("Unexpected '{}'. Maybe the proof footer has the wrong order. Expected order of the footer is 'output', 'conclusion', and finally 'end pseudo-Boolean proof'.", .0)]
    WrongFooterOrder(&'static str),

    #[error(
        "Cutting planes step ('pol') ended with constraint stack size {0}. The stack should only contain 1 constraint at the end of the step."
    )]
    StackNotOne(usize),

    #[error(
        "While checking the cutting planes step ('pol'), an instruction tried to access more constraints than there are constraints on the constraint stack."
    )]
    NotEnoughConstraintsOnStack,

    #[error("The constraint is not implied by reverse unit propagation (RUP) from {} database. Rerun with the option '--trace-failed' to see the propagations.", if *.0 {"only core"} else {"core and derived"})]
    NotRUP(bool),

    #[error("The constraint with ID {} is not contradicting, as specified by the hint.", .0.to_string())]
    HintNoContradiction(isize),

    #[error("There is no contradicting constraint in the database.")]
    NoContradicitionInDB,

    #[error(
        "Unable to verify 'conclusion UNSAT'. The input problem contains a contradiction, but 'conclusion UNSAT' can only be used without an objective. Use 'conclusion BOUNDS INF INF' for infeasible optimization problems."
    )]
    ConclusionUnsatObjective,

    #[error(
        "A solution has been logged in the proof using `sol`, `soli`, or `solx`. Hence, the conclusion `UNSAT` is wrong."
    )]
    ConclusionUnsatSolutionLogged,

    #[error("Unable to verify 'conclusion UNSAT'.")]
    ConclusionUnsat,

    #[error("The proof is missing the 'end pseudo-Boolean proof' line.")]
    EndProofMissing,

    #[error(
        "The solution logging rule that introduces a solution improving constraint ('soli') requires an objective, but no objective has been defined in the input problem."
    )]
    SolutionImprovingRequiresObjective,

    #[error(
        "Can not check optional objective value against objective value of solution, since no objective function is given."
    )]
    SolutionValueHintNoObjective,

    #[error("The logged solution was claimed to have value {0} but it has value {1}.")]
    SolutionIncorrectValue(BigInt, BigInt),

    #[error(
        "The objective value logging rule that introduces a solution improving constraint ('obji') requires an objective, but no objective has been defined in the input problem."
    )]
    ObjectiveImprovingRequiresObjective,

    #[error(
        "The given solution does not propagate to a complete assignment of all the currently used variables. The variable '{0}' is not assigned."
    )]
    SolutionNotComplete(String),

    #[error("The given assignment falsifies the constraint with ID {}.", .0.to_string())]
    SolutionFalsifyingConstraint(usize),

    #[error("The propagated assignment does not satisfy the constraint with ID {}.", .0.to_string())]
    SolutionNotSatisfiedConstraint(usize),

    #[error(
        "The 'conclusion SAT' can only be used for decision instances, but the input problem contains an objective."
    )]
    ConclusionSatWithObjective,

    #[error(
        "Unable to verify 'conclusion {0}'. No solution has been logged in the proof and no solution has been given in the conclusion."
    )]
    SolutionMissing(String),

    #[error(
        "Unable to verify conclusion. The solution given as hint for this conclusion falsifies a constraint in the input instance.\n\nHint: Note that the solution given for the conclusion is not propagated. If the solution should be propagated, then log the solution inside the proof using either 'sol', 'soli', or 'solx'."
    )]
    OriginalConstraintNotSatisfied,

    #[error(
        "The solution hint specified for the conclusion assigns the same variable to true and false. I.e., the solution contains both the literal and its negation."
    )]
    ConclusionSolutionConflicting,

    #[error(
        "Unable to verify 'conclusion BOUNDS'. The input problem does not contain an objective, which is required for 'conclusion BOUNDS'."
    )]
    ConclusionBoundsNoObjective,

    #[error(
        "Unable to verify 'conclusion BOUNDS'. The lower bounding constraint is not syntactically implied by any constraint in the database."
    )]
    ConclusionBoundsLBNotImpliedByDB,

    #[error(
        "Unable to verify 'conclusion BOUNDS'. The claimed upper bound of {0} mismatches the best recorded upper bound of {1}."
    )]
    ConclusionBoundsUpperBoundMismatchRecorded(BigInt, BigInt),

    #[error(
        "Unable to verify 'conclusion BOUNDS'. The claimed upper bound of {0} mismatches the objective value of the hint, which is {1}."
    )]
    ConclusionBoundsUpperBoundMismatchHint(BigInt, BigInt),

    #[error(
        "Unable to verify 'conclusion BOUNDS'. The problem is claimed to be infeasible and to have a upper bound at the same time. When the problem has a lower bound, then it is feasible."
    )]
    ConclusionBoundsInfeasibleAndUpper,

    #[error(
        "The 'conclusion BOUNDS' claims that the problem is infeaible, but at least one solution has been logged."
    )]
    ConclusionBoundsInfeasibleAndSolutionLogged,

    #[error(
        "The lower bound claimed for `conclusion BOUNDS` is larger than the best logged objective value."
    )]
    ConclusionBoundsLBLargerBestSolution,

    #[error(
        "Unable to verify 'conclusion BOUNDS'. The claimed lower bound of {0} is larger than the claimed upper bound of {1}."
    )]
    ConclusionBoundsLowerGreaterUpper(BigInt, BigInt),

    #[error(
        "A variable in the objective is not assigned by the solution. Please explicitly specify a value for this variable."
    )]
    ObjectiveUnassigned,

    #[error(
        "Proofgoal {} could not be autoproven. Please add an explicit subproof for proofgoal {}.", .0.to_string(), .0.to_string()
    )]
    AutoprovingFailed(ProofgoalID),

    #[error("Proofgoal {} could not be autoproven for checked deletion. Please add an explicit subproof for proofgoal {}.", .0.to_string(), .0.to_string())]
    CheckedDeletionAutoprovingFailed(ProofgoalID, Option<usize>),

    #[error(
        "The end of the proofgoal subproof does not a constraint ID pointing to a constradiction and it was not possible to find the contradiction in the subproof."
    )]
    ProofgoalEndContradicitionNotObvious,

    #[error("There is no open subcontext to end here.")]
    NoOpenSubcontext,

    #[error(
        "The 'proofgoal' rule can only be used inside a subproof, but no subproof is activated."
    )]
    ProofgoalOutsideSubproof,

    #[error("The internal proofgoal with ID #{} does not exist.", .0.to_string())]
    InternalProofgoalNotExisting(usize),

    #[error("The internal proofgoal with ID #{} is already proven.", .0.to_string())]
    InternalProofgoalAlreadyProven(usize),

    #[error("The database proofgoal with ID {} does not exist or has already been proven.", .0.to_string())]
    DatabaseProofgoalNotExisting(usize),

    #[error(
        "It is not possible to start another proofgoal subproof while the previous proofgoal subproof is not finished."
    )]
    StartProofgoalWhileInsideProofgoal,

    #[error(
        "There are still open subcontext at the end of the proof, which is started by the output rule."
    )]
    OutputWhileOpenSubcontext,

    #[error("Rule cannot be used within a subproof")]
    RuleNotSubproofFriendly(String),

    #[error("Only pseudo-Boolean proofs of version 2.0 or greater are supported.")]
    UnsupportedProofVersion,

    #[error("Deletion of derived constraint ID {} using deletion from core set.", .0.to_string())]
    DeletionFromCoreDeletesDerived(usize),

    #[error("Deletion of core constraint ID {} using deletion from derived set.", .0.to_string())]
    DeletionFromDerivedDeletesCore(usize),

    #[error("Constraint is in database, but is expected to be deleted.")]
    DeletedConstraintInDB,

    #[error(
        "Usage of dominance-based strengthening without order an objective. Dominance-based strengthening can only be used when an order is loaded or the problem contains an objective."
    )]
    DominanceNoObjectiveOrOrder,

    #[error("The objective update rule cannot be used without an objective.")]
    ObjectiveUpdateNoObjective,

    #[error(
        "The keyword 'vars' to specify variables can only be used inside the 'def_order' or the 'transitivity'."
    )]
    VarsNotAllowedHere,

    #[error(
        "The keyword 'spec' for the order specification can only be used inside the 'def_order'."
    )]
    SpecNotAllowedHere,

    #[error("The keyword 'def' to specify variables can only be used inside the 'def_order'.")]
    DefNotAllowedHere,

    #[error("It is not allowed to end this block with a constraint ID hint.")]
    EndNoHintAllowed,

    #[error(
        "The rules 'left', 'right', and 'aux' are only allowed in the variable definition of an order."
    )]
    RuleOnlyAllowedInOrderVars,

    #[error("Variables in 'left', 'right' and 'aux' must occur exactly once")]
    OrderVariablesNonDistinct,

    #[error("Variable lists in 'left' and 'right' must have the same length")]
    AsymmetricOrderVariables,

    #[error(
        "Constraints in 'def' and 'spec' are only allowed to contain variables from 'left', 'right' and 'aux'."
    )]
    UndeclaredVariablesInOrder,

    #[error("Witnesses inside 'spec' can only map variables in 'aux'")]
    SpecWitnessMapsNonAuxVariable,

    #[error(
        "The rules 'fresh_right' is only allowed in the variable definition of the transitivity proof of the order definition."
    )]
    RuleOnlyAllowedInTransitivityVars,

    #[error("The 'aux' vars for an order has to be empty in version 2.0 of the proof format.")]
    AuxVarsNonEmpty,

    #[error("Constraints without a rule are only allowed in the 'def' of the 'def_order'.")]
    OnlyAllowedInOrderDef,

    #[error(
        "The environment 'proof' is only allowed in the 'reflexivity' or 'transitivity' rules of the order definition."
    )]
    ProofOnlyInReflexivityOrTransitivity,

    #[error("Reflexivity has already been proven.")]
    ReflexivityAlreadyProven,

    #[error("Transitivity has already been proven.")]
    TransitivityAlreadyProven,

    #[error("There was no 'proof' for reflexivity presented inside 'reflexivity' environment.")]
    ReflexivityProofMissing,

    #[error("There was no 'proof' for transitivity presented inside 'transitivity' environment.")]
    TransitivityProofMissing,

    #[error("Reflexivity of the order could not be proven.")]
    ReflexivityProofFailed,

    #[error("Transitivity of the order could not be proven.")]
    TransitivityProofFailed,

    #[error("The order '{0}' can not be loaded, as it is not defined.")]
    OrderNotDefined(String),

    #[error(
        "The order is defined over {0} variables, but 'load_order' is trying to load it with {1} variables."
    )]
    OrderWrongNumberVariables(usize, usize),

    #[error("The 'scope' subcontext can only be used inside subproofs.")]
    ScopeOutsideSubproof,

    #[error("The scope named {0} does not exist.")]
    ScopeDoesNotExist(String),

    #[error(
        "The previous scope has not ended. End the previous scope before opening a new scope."
    )]
    StartScopeWhileOtherScopeOpen,

    #[error("The proofgoal {0} can not be used in scope {1}.")]
    ProofgoalNotInScope(ProofgoalID, ScopeId),

    #[error(
        "The 'left' variables of the order are undefined. Please add 'left' to the 'vars' definition of the order."
    )]
    LeftVarsUndefined,

    #[error(
        "The 'right' variables of the order are undefined. Please add 'right' to the 'vars' definition of the order."
    )]
    RightVarsUndefined,

    #[error(
        "The 'aux' variables of the order are undefined. Please add 'aux' to the 'vars' definition of the order."
    )]
    AuxVarsUndefined,

    #[error("Proof by contradiction should be able to show contradiction by RUP")]
    ProofByContradictionNotRUP,

    #[error("Checked deletion failed and `--force-checked-deletion` option used. Proofgoal with ID {} could not be autoproven.", .0.to_string())]
    ForceCheckedDeletionFailed(ProofgoalID),

    #[error("Unchecked deletion with a subproof is currently not allowed.")]
    UncheckedWithSubproof,

    #[error("Deletion with subproof can only be used if exactly one core constraint is deleted.")]
    DeletionSubproofNotOneCoreConstraint,

    #[error("Multiple core constraints can only be deleted at once if the witness is empty.")]
    DeletionMultipleCoreWithWitness,

    #[error(
        "Checked deletion with a witness cannot be used while strengthening to core is activated."
    )]
    DeletionWithWitnessWhileStrengtheningToCore,

    #[error("Subproof can only use core constraints, but ID {} is not a core constraint.", .0.to_string())]
    CoreSubproofUsingNonCoreConstraint(isize),

    #[error("Equals objective rule `eobj` can only be used if there is an objective.")]
    EqualObjectiveWithoutObjective,

    #[error("The specified objective is not equal to the current objective.")]
    ObjectivesNotEqual,

    #[error(
        "Output: No output formula file has been specified. The output type `FILE` expects that the third positional command line argument is the path to the output formula."
    )]
    NoOutputFormulaGiven,

    #[error("Output: The current objective does not match the output formula objective.")]
    OutputObjectiveMismatch,

    #[error(
        "Output: The output formula contains an objective, but the problem at the end of the proof does not contain an objective."
    )]
    OutputObjectiveButNoProofObjective,

    #[error(
        "Output: The problem at the end of the proof contains an objective, but the output formula does not contain an objective."
    )]
    OutputNoObjectiveButProofObjective,

    #[error(
        "Output: A constraint in the output formula is not in the database at the end of the proof."
    )]
    OutputConstraintNotInDatabase(Sticky<Rc<DBConstraint>>),

    #[error("Output: The constraint with ID {} is in the output formula, but in the database it is not a core constraint. The constraint {} should be moved to the core set, as the output section requires equivalence between the output formula and the core constraints.", .0.to_string(), .0.to_string())]
    OutputConstraintNotInCore(usize),

    #[error("Output: The core constraint with ID {} is not in the output formula.", .0.to_string())]
    OutputCoreConstraintNotInOutput(usize),

    #[error(
        "Output: The guarantee EQUIOPTIMAL can only be used if the proof contains an objective."
    )]
    OutputEquioptimalWithoutObjective,

    #[error(
        "Output: The guarantee EQUISATISFIABLE can only be used if the proof does not contain an objective."
    )]
    OutputEquisatisfiableWithObjective,

    #[error("Proof contains `fail` rule.")]
    FailProofUsed,

    #[error(
        "The given solution is conflicting in itself. A variable has been assigned to true and false in the same solution."
    )]
    SolutionIsConflicting,
}

impl CheckingError {
    pub fn not_implemented(rule: &str) -> Self {
        CheckingError::NotImplemented {
            rule: rule.to_string(),
        }
    }

    pub fn deletion(constraint_id: usize, reason: &str) -> Self {
        CheckingError::FailedDeletion {
            constraint_id,
            reason: reason.to_string(),
        }
    }

    pub fn not_found(constraint: &Rc<DBConstraint>, only_core: bool) -> Self {
        if only_core {
            CheckingError::NotFoundInDB {
                constraint: Sticky::new(constraint.clone()),
                which_database: " core".to_string(),
            }
        } else {
            CheckingError::NotFoundInDB {
                constraint: Sticky::new(constraint.clone()),
                which_database: String::new(),
            }
        }
    }

    pub fn not_equal(
        expected_constraint: &Rc<DBConstraint>,
        hint_constraint: &Rc<DBConstraint>,
    ) -> Self {
        CheckingError::NotEqual {
            expected_constraint: Sticky::new(expected_constraint.clone()),
            hint_constraint: Sticky::new(hint_constraint.clone()),
        }
    }

    pub fn not_implied_db(constraint: &Rc<DBConstraint>, only_core: bool) -> Self {
        if only_core {
            CheckingError::NotImpliedDB {
                constraint: Sticky::new(constraint.clone()),
                which_database: " core".to_string(),
            }
        } else {
            CheckingError::NotImpliedDB {
                constraint: Sticky::new(constraint.clone()),
                which_database: String::new(),
            }
        }
    }

    pub fn not_implied(
        expected_constraint: &Rc<DBConstraint>,
        hint_constraint: &Rc<DBConstraint>,
    ) -> Self {
        CheckingError::NotImplied {
            expected_constraint: Sticky::new(expected_constraint.clone()),
            hint_constraint: Sticky::new(hint_constraint.clone()),
        }
    }

    pub fn not_implied_substituted_db(constraint: &Rc<DBConstraint>, only_core: bool) -> Self {
        if only_core {
            CheckingError::NotImpliedDBSubstituted {
                constraint: Sticky::new(constraint.clone()),
                which_database: " core".to_string(),
            }
        } else {
            CheckingError::NotImpliedDBSubstituted {
                constraint: Sticky::new(constraint.clone()),
                which_database: String::new(),
            }
        }
    }

    pub fn not_implied_substituted(
        expected_constraint: &Rc<DBConstraint>,
        hint_constraint: &Rc<DBConstraint>,
    ) -> Self {
        CheckingError::NotImpliedSubstituted {
            expected_constraint: Sticky::new(expected_constraint.clone()),
            hint_constraint: Sticky::new(hint_constraint.clone()),
        }
    }

    pub fn out_of_bounds(index: isize, lower: isize, upper: isize) -> Self {
        CheckingError::OutOfBoundsAccess {
            index,
            lower,
            upper,
        }
    }

    pub fn access_deleted(index: isize) -> Self {
        CheckingError::AccessingDeleted { index }
    }

    pub fn formula_size_mismatch(formula_size: usize, expected_size: usize) -> Self {
        CheckingError::FormulaSizeMismatch {
            formula_size,
            expected_size,
        }
    }

    pub fn add_context(self, file: PathBuf, line: usize) -> VeriPBError {
        VeriPBError::Checking {
            error: self,
            file,
            line: line + 1,
        }
    }
}
