use std::{
    fmt::{Display, Formatter},
    rc::Rc,
    str::FromStr,
};

use logos::{Lexer, Logos};
use malachite_bigint::BigInt;
use veripb_formula::{pb_constraint::constraint_from_terms, prelude::*};
use veripb_parser::{assignment_parser::parse_bool_assignment_to_raw_vec, error::ParserError};

use crate::prelude::*;

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum Bound {
    Bounded(BigInt),
    Unbounded,
}

impl Bound {
    #[inline]
    pub fn unwrap(self) -> BigInt {
        match self {
            Self::Bounded(bound) => bound,
            Self::Unbounded => {
                panic!("Cannot unwrap an unbounded bound to the value of the bound!")
            }
        }
    }
}

impl Display for Bound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Bound::Unbounded => write!(f, "INF")?,
            Bound::Bounded(bound) => write!(f, "{bound}")?,
        }

        Ok(())
    }
}

#[derive(Debug, Logos)]
#[logos(skip r"[ \t\r\n]")]
enum ConclusionData {
    #[token(":")]
    Hint,

    #[regex("[+-]?[0-9]+", |lex| Bound::Bounded(BigInt::from_str(lex.slice()).unwrap())) ]
    #[token("INF", |_lex| Bound::Unbounded)]
    Bound(Bound),
}

#[derive(Debug, Logos)]
#[logos(skip r"[ \t\r\n]")]
pub enum ConclusionResult {
    #[token("SAT")]
    Satisfiable,

    #[token("UNSAT")]
    Unsatisfiable,

    #[token("BOUNDS")]
    Bounds,

    #[token("NONE")]
    None,
}

impl Display for ConclusionResult {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(
            f,
            "{}",
            match self {
                Self::Satisfiable => "SAT",
                Self::Unsatisfiable => "UNSAT",
                Self::Bounds => "BOUNDS",
                Self::None => "NONE",
            }
        )
    }
}

#[derive(Debug)]
pub struct ConclusionRule {
    result: ConclusionResult,
    lower_bound: Option<Bound>,
    upper_bound: Option<Bound>,
    constraint_id_hint: Option<isize>,
    solution_hint: Option<Vec<Lit>>,
    propagated_solution: Option<Assignment<BooleanVar>>,
}

impl ConclusionRule {
    #[inline]
    pub fn new(
        result: ConclusionResult,
        lower_bound: Option<Bound>,
        upper_bound: Option<Bound>,
        constraint_id_hint: Option<isize>,
        solution_hint: Option<Vec<Lit>>,
    ) -> Self {
        ConclusionRule {
            result,
            lower_bound,
            upper_bound,
            constraint_id_hint,
            solution_hint,
            propagated_solution: None,
        }
    }

    pub fn parse(lex: Lexer<RuleToken>, context: &mut Context) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        match lex.next() {
            Some(Ok(ConclusionResult::None)) => Ok(ConclusionRule::new(
                ConclusionResult::None,
                None,
                None,
                None,
                None,
            )),
            Some(Ok(ConclusionResult::Unsatisfiable)) => {
                let mut rule =
                    ConclusionRule::new(ConclusionResult::Unsatisfiable, None, None, None, None);
                let mut lex = lex.morph();
                match lex.next() {
                    Some(Ok(ConclusionData::Hint)) => {
                        rule.constraint_id_hint = Some(IntegerToken::parse(&mut lex.morph())?)
                    }
                    None => {}
                    _ => {
                        return Err(ParserError::token_error(
                            lex.span(),
                            "':' to start hint or end-of-line",
                        ));
                    }
                }
                Ok(rule)
            }
            Some(Ok(ConclusionResult::Satisfiable)) => {
                let mut rule =
                    ConclusionRule::new(ConclusionResult::Satisfiable, None, None, None, None);
                let mut lex = lex.morph();
                match lex.next() {
                    Some(Ok(ConclusionData::Hint)) => {
                        rule.solution_hint = Some(parse_bool_assignment_to_raw_vec(
                            &mut lex.morph(),
                            &mut context.var_names,
                        )?);
                    }
                    None => {}
                    _ => {
                        return Err(ParserError::token_error(
                            lex.span(),
                            "':' to start hint or end-of-line",
                        ));
                    }
                }
                Ok(rule)
            }
            Some(Ok(ConclusionResult::Bounds)) => {
                let mut rule =
                    ConclusionRule::new(ConclusionResult::Bounds, None, None, None, None);
                let mut lex = lex.morph();
                match lex.next() {
                    Some(Ok(ConclusionData::Bound(bound))) => rule.lower_bound = Some(bound),
                    _ => {
                        return Err(ParserError::token_error(
                            lex.span(),
                            "integer or 'INF' for lower bound",
                        ));
                    }
                }
                rule.upper_bound = match lex.next() {
                    Some(Ok(ConclusionData::Bound(bound))) => Some(bound),
                    Some(Ok(ConclusionData::Hint)) => {
                        let mut lex_inner = lex.morph();
                        rule.constraint_id_hint = Some(IntegerToken::parse(&mut lex_inner)?);
                        let mut lex_inner = lex_inner.morph();
                        if let Some(Ok(ConclusionData::Bound(bound))) = lex_inner.next() {
                            lex = lex_inner.morph();
                            Some(bound)
                        } else {
                            return Err(ParserError::token_error(
                                lex_inner.span(),
                                "constraint ID for lower bounding constraint",
                            ));
                        }
                    }
                    _ => {
                        return Err(ParserError::token_error(
                            lex.span(),
                            "':' for hint or integer or 'INF' for upper bound",
                        ));
                    }
                };
                match lex.next() {
                    Some(Ok(ConclusionData::Hint)) => {
                        rule.solution_hint = Some(parse_bool_assignment_to_raw_vec(
                            &mut lex.morph(),
                            &mut context.var_names,
                        )?);
                    }
                    None => {}
                    _ => {
                        return Err(ParserError::token_error(
                            lex.span(),
                            "':' to start hint or end-of-line",
                        ));
                    }
                }
                Ok(rule)
            }
            _ => Err(ParserError::token_error(
                lex.span(),
                "conclusion result 'NONE', 'UNSAT', 'SAT', or 'BOUNDS'",
            )),
        }
    }

    /// Checks if the database contains a contradiction.
    #[inline]
    fn check_contradiction(&self, database: &Database) -> Result<isize, CheckingError> {
        match self.constraint_id_hint {
            None => {
                if let Some(contradicting_id) = database.contains_contradiction() {
                    Ok(contradicting_id as isize)
                } else {
                    Err(CheckingError::NoContradicitionInDB)
                }
            }
            Some(hint) => {
                let constraint = database.get_entry(hint)?;
                if constraint.is_contradicting() {
                    return Ok(database.normalize_id(hint));
                }
                Err(CheckingError::HintNoContradiction(hint))
            }
        }
    }

    /// Check if the proof contains a solution or the hint is a correct solution.
    ///
    /// Returns the propagated solution if a solution was given as a hint.
    #[inline]
    fn check_solution(
        &self,
        context: &mut Context,
    ) -> Result<Option<Assignment<BooleanVar>>, CheckingError> {
        match &self.solution_hint {
            Some(solution) => {
                // Do not propagate the assignment, as the hint given in the conclusion has to be a full assignment.
                let assignment = Assignment::from(solution)
                    .ok_or(CheckingError::ConclusionSolutionConflicting)?;
                for constraint in context.original_constraints.iter() {
                    if !constraint.is_satisfied(&assignment) {
                        return Err(CheckingError::OriginalConstraintNotSatisfied);
                    }
                }
                Ok(Some(assignment))
            }
            None => {
                if context.best_valid_objective_value.is_none() {
                    return Err(CheckingError::SolutionMissing(self.result.to_string()));
                }
                Ok(None)
            }
        }
    }
}

impl Rule for ConclusionRule {
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        // Check order of footer rules.
        if context.has_conclusion {
            return Err(CheckingError::DoubleConclusion);
        }
        if !context.has_output || context.has_end_proof {
            return Err(CheckingError::WrongFooterOrder("conclusion"));
        }

        // Perform the necessary checks for each conclusion.
        match self.result {
            ConclusionResult::None => {}
            ConclusionResult::Unsatisfiable => {
                if context.best_objective_value.is_some() {
                    return Err(CheckingError::ConclusionUnsatSolutionLogged);
                }
                if context.objective.is_some() {
                    return Err(CheckingError::ConclusionUnsatObjective);
                }

                self.constraint_id_hint = Some(self.check_contradiction(database)?);
            }
            ConclusionResult::Satisfiable => {
                if context.objective.is_some() {
                    return Err(CheckingError::ConclusionSatWithObjective);
                }

                self.propagated_solution = self.check_solution(context)?;
            }
            ConclusionResult::Bounds => {
                if context.objective.is_none() {
                    return Err(CheckingError::ConclusionBoundsNoObjective);
                }

                // Check lower bound.
                match self.lower_bound.as_ref().unwrap() {
                    Bound::Unbounded => {
                        if self.upper_bound != Some(Bound::Unbounded) {
                            return Err(CheckingError::ConclusionBoundsInfeasibleAndUpper);
                        }
                        if context.best_objective_value.is_some() {
                            return Err(CheckingError::ConclusionBoundsInfeasibleAndSolutionLogged);
                        }

                        self.constraint_id_hint = Some(self.check_contradiction(database)?);
                    }
                    Bound::Bounded(bound) => {
                        // Check that lower bound is not better than best logged solution.
                        if context
                            .best_objective_value
                            .as_ref()
                            .is_some_and(|best| bound > best)
                        {
                            return Err(CheckingError::ConclusionBoundsLBLargerBestSolution);
                        }
                        // Construct lower bounding constraint.
                        let constraint = constraint_from_terms::<BigInt>(
                            context
                                .objective
                                .as_ref()
                                .unwrap()
                                .terms
                                .values()
                                .cloned()
                                .collect(),
                            bound.to_owned() - &context.objective.as_ref().unwrap().constant,
                        );
                        let db_constraint = Rc::new(DBConstraint::from(constraint));

                        if let Some(contradicting_id) = database.contains_contradiction() {
                            self.constraint_id_hint = Some(contradicting_id as isize);
                        } else {
                            self.constraint_id_hint = Some(check_implication(
                                context,
                                database,
                                &db_constraint,
                                self.constraint_id_hint,
                            )?);
                        }
                    }
                }

                // Check upper bound.
                match self.upper_bound.as_ref().unwrap() {
                    Bound::Unbounded => {}
                    Bound::Bounded(bound) => {
                        if self.lower_bound > self.upper_bound {
                            return Err(CheckingError::ConclusionBoundsLowerGreaterUpper(
                                self.lower_bound.to_owned().unwrap().unwrap(),
                                self.upper_bound.to_owned().unwrap().unwrap(),
                            ));
                        }

                        match self.check_solution(context)? {
                            Some(assignment) => {
                                let value = context
                                    .original_objective
                                    .as_ref()
                                    .unwrap()
                                    .evaluate(&assignment)
                                    .ok_or(CheckingError::ObjectiveUnassigned)?;
                                if *bound != value {
                                    return Err(
                                        CheckingError::ConclusionBoundsUpperBoundMismatchHint(
                                            bound.to_owned(),
                                            value,
                                        ),
                                    );
                                }
                                self.propagated_solution = Some(assignment);
                            }
                            None => {
                                if bound != context.best_valid_objective_value.as_ref().unwrap() {
                                    return Err(
                                        CheckingError::ConclusionBoundsUpperBoundMismatchRecorded(
                                            bound.to_owned(),
                                            context.best_valid_objective_value.clone().unwrap(),
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Print the verification statement for each conclusion.
        if context.args.print_verification_result {
            match self.result {
                ConclusionResult::None => println!("s VERIFIED NO CONCLUSION"),
                ConclusionResult::Unsatisfiable => println!("s VERIFIED UNSATISFIABLE"),
                ConclusionResult::Satisfiable => println!("s VERIFIED SATISFIABLE"),
                ConclusionResult::Bounds => println!(
                    "s VERIFIED BOUNDS {} <= obj <= {}",
                    self.lower_bound.as_ref().unwrap(),
                    self.upper_bound.as_ref().unwrap()
                ),
            }
        }
        let result_string = match self.result {
            ConclusionResult::None => "VERIFIED NO CONCLUSION".to_string(),
            ConclusionResult::Unsatisfiable => "VERIFIED UNSATISFIABLE".to_string(),
            ConclusionResult::Satisfiable => "VERIFIED SATISFIABLE".to_string(),
            ConclusionResult::Bounds => format!(
                "VERIFIED BOUNDS {} <= obj <= {}",
                self.lower_bound.as_ref().unwrap(),
                self.upper_bound.as_ref().unwrap()
            ),
        };
        context.verification_result = Some(result_string);

        context.has_conclusion = true;
        Ok(vec![])
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.write("conclusion ");
        match self.result {
            ConclusionResult::Satisfiable => {
                elaborator.write("SAT");
                if let Some(assignment) = self.propagated_solution.as_ref() {
                    elaborator.write(" : ");
                    elaborator.write(&assignment.to_pretty_string(&context.var_names));
                }
            }
            ConclusionResult::Unsatisfiable => {
                elaborator.write("UNSAT : ");
                let index = self.constraint_id_hint.unwrap();
                let constraint = database
                    .get_entry_usize(index as usize)
                    .expect("constraint already accessed before");
                elaborator.write(
                    &constraint
                        .get_out_id(index as usize)
                        .expect("constraint should have output ID")
                        .to_string(),
                );
            }
            ConclusionResult::Bounds => {
                elaborator.write("BOUNDS ");
                elaborator.write(&self.lower_bound.as_ref().unwrap().to_string());
                elaborator.write(" : ");
                let index = self.constraint_id_hint.unwrap();
                let constraint = database
                    .get_entry_usize(index as usize)
                    .expect("constraint already accessed before");
                elaborator.write(
                    &constraint
                        .get_out_id(index as usize)
                        .expect("constraint should have output ID")
                        .to_string(),
                );
                elaborator.write(" ");
                elaborator.write(&self.upper_bound.as_ref().unwrap().to_string());
                if self.upper_bound != Some(Bound::Unbounded) {
                    if let Some(assignment) = self.propagated_solution.as_ref() {
                        elaborator.write(" : ");
                        elaborator.write(&assignment.to_pretty_string(&context.var_names));
                    }
                }
            }
            ConclusionResult::None => elaborator.write("NONE"),
        }
        elaborator.writeln(";");
        Ok(())
    }
}
