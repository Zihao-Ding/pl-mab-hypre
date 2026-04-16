//! Parser for the OPB format as [specified by the PB competition 2024](https://www.cril.univ-artois.fr/PB24/OPBgeneral.pdf).
//!
//! Tokenization is performed using [`OPBToken`].

use std::path::Path;

use ahash::AHashMap;
use logos::{Lexer, Logos};
use malachite_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};
use veripb_formula::prelude::*;

use crate::{error::ParserError, opb_token::OPBToken, parser::get_lines};

type OptionalConstraint = Option<PBConstraintEnum>;
type Constraint = PBConstraintEnum;

/// Supported comparison operators used for parsing pseudo-Boolean constraints.
#[derive(Debug, PartialEq, Clone, Copy)]
enum Comparator {
    GreaterEqual,
    LessEqual,
    Equal,
}

/// Parse an OPB file into [`Formula`] of constraints and store the names of the variables in a [`VarNameManager`].
///
/// This function creates a [`VarNameManager`] and calls the function `parse_opb_from_file_given_var_manager()`. See `parse_opb_from_file_given_var_manager()` for more details.
#[inline]
pub fn parse_opb_from_file<P>(
    filename: P,
) -> Result<(Formula, VarNameManager, AHashMap<String, isize>), ParserError>
where
    P: AsRef<Path>,
{
    let mut var_name_manager = VarNameManager::default();
    let (formula, labels) = parse_opb_from_file_given_var_manager(filename, &mut var_name_manager)?;
    Ok((formula, var_name_manager, labels))
}

/// Parse an OPB file into [`Formula`] of constraints and store the names of the variables in a [`VarNameManager`].
///
/// Furthermore, labels for constraints are supported. The labels are returned as a mapping from a [`String`] to a constraint ID. The constraint IDs start at 1, hence the the constraint ID is offset by 1 to the constraints in the formula.
pub fn parse_opb_from_file_given_var_manager<P>(
    filename: P,
    var_name_manager: &mut VarNameManager,
) -> Result<(Formula, AHashMap<String, isize>), ParserError>
where
    P: AsRef<Path>,
{
    let mut formula = Formula::default();
    let mut labels = AHashMap::new();
    let lines = get_lines(&filename)?;

    for (line_number, line) in lines.map_while(Result::ok).enumerate() {
        let mut lex = OPBToken::lexer(&line);
        let result = match lex.next() {
            None | Some(Ok(OPBToken::Comment)) => continue,
            Some(Ok(OPBToken::Label)) => {
                let label = lex.slice();
                let result = match lex.next() {
                    Some(Ok(OPBToken::Integer)) => Some(
                        parse_opb_constraint_dyn(&mut lex, var_name_manager).map_err(|e| {
                            e.add_file_and_line(
                                filename.as_ref().to_string_lossy().to_string(),
                                line_number,
                            )
                            .unwrap()
                        })?,
                    ),
                    Some(Ok(OPBToken::GreaterEqual)) => Some(
                        parse_constraint_empty_terms(&mut lex, Comparator::GreaterEqual).map_err(
                            |e| {
                                e.add_file_and_line(
                                    filename.as_ref().to_string_lossy().to_string(),
                                    line_number,
                                )
                                .unwrap()
                            },
                        )?,
                    ),
                    Some(Err(_)) | Some(Ok(_)) | None => {
                        return Err(ParserError::token_error_with_file(
                            lex.span(),
                            "'>=' or integer",
                            filename.as_ref().to_string_lossy().to_string(),
                            line_number,
                        ));
                    }
                };
                if result.as_ref().unwrap().1.is_some() {
                    return Err(ParserError::token_error_with_file(
                        lex.span(),
                        "inequality constraint",
                        filename.as_ref().to_string_lossy().to_string(),
                        line_number,
                    ));
                }
                labels.insert(label.to_string(), (formula.len() + 1) as isize);
                result
            }
            Some(Ok(OPBToken::Minimize)) => {
                formula.objective = Some(parse_opb_objective(&mut lex, var_name_manager, false)?);
                None
            }
            Some(Ok(OPBToken::Maximize)) => {
                formula.objective = Some(parse_opb_objective(&mut lex, var_name_manager, true)?);
                None
            }
            Some(Ok(OPBToken::Integer)) => Some(
                parse_opb_constraint_dyn(&mut lex, var_name_manager).map_err(|e| {
                    e.add_file_and_line(
                        filename.as_ref().to_string_lossy().to_string(),
                        line_number,
                    )
                    .unwrap()
                })?,
            ),
            Some(Ok(OPBToken::GreaterEqual)) => Some(
                parse_constraint_empty_terms(&mut lex, Comparator::GreaterEqual).map_err(|e| {
                    e.add_file_and_line(
                        filename.as_ref().to_string_lossy().to_string(),
                        line_number,
                    )
                    .unwrap()
                })?,
            ),
            Some(Ok(OPBToken::LessEqual)) => Some(
                parse_constraint_empty_terms(&mut lex, Comparator::LessEqual).map_err(|e| {
                    e.add_file_and_line(
                        filename.as_ref().to_string_lossy().to_string(),
                        line_number,
                    )
                    .unwrap()
                })?,
            ),
            Some(Ok(OPBToken::Equal)) => Some(
                parse_constraint_empty_terms(&mut lex, Comparator::Equal).map_err(|e| {
                    e.add_file_and_line(
                        filename.as_ref().to_string_lossy().to_string(),
                        line_number,
                    )
                    .unwrap()
                })?,
            ),
            Some(Err(_)) | Some(Ok(_)) => {
                return Err(ParserError::token_error_with_file(
                    lex.span(),
                    "'*', 'min:', '>=', '<=', '=', or integer",
                    filename.as_ref().to_string_lossy().to_string(),
                    line_number,
                ));
            }
        };
        if let Some((geq_constraint, leq_constraint)) = result {
            formula.constraints.push(geq_constraint);
            if let Some(constraint) = leq_constraint {
                formula.constraints.push(constraint);
            }
        }
    }

    Ok((formula, labels))
}

/// Parse an objective specified in OPB format.
pub fn parse_opb_objective(
    lex: &mut Lexer<OPBToken>,
    var_name_manager: &mut VarNameManager,
    is_maximization: bool,
) -> Result<PBObjective, ParserError> {
    let mut terms = Vec::new();
    let mut integer = BigInt::zero();
    match lex.next() {
        Some(Ok(OPBToken::Integer)) => integer = lex.slice().parse().unwrap(),
        Some(Ok(OPBToken::Semicolon)) | None => {
            return Ok(PBObjective::from_terms(terms, integer, is_maximization))
        }
        _ => return Err(ParserError::token_error(lex.span(), "integer or ';'")),
    }

    while let Some(token) = lex.next() {
        // Expect the next literal.
        let lit = match token {
            Ok(OPBToken::Var) => Lit::from_var(var_name_manager.add_by_name(lex.slice()), false),
            Ok(OPBToken::Negation) => {
                if lex.next() == Some(Ok(OPBToken::Var)) {
                    Lit::from_var(var_name_manager.add_by_name(lex.slice()), true)
                } else {
                    return Err(ParserError::token_error(lex.span(), "variable name"));
                }
            }
            Ok(OPBToken::Semicolon) => {
                return Ok(PBObjective::from_terms(terms, integer, is_maximization));
            }
            _ => return Err(ParserError::token_error(lex.span(), "literal or ';'")),
        };

        terms.push(GeneralPBTerm::new(integer, lit));

        // Expect the next integer or end.
        match lex.next() {
            Some(Ok(OPBToken::Integer)) => integer = lex.slice().parse().unwrap(),
            Some(Ok(OPBToken::Semicolon)) | None => {
                return Ok(PBObjective::from_terms(
                    terms,
                    BigInt::zero(),
                    is_maximization,
                ))
            }
            _ => return Err(ParserError::token_error(lex.span(), "integer or ';'")),
        }
    }

    Ok(PBObjective::from_terms(terms, integer, is_maximization))
}

/// Parse a single OPB constraint.
pub fn parse_single_constraint(
    lex: &mut Lexer<OPBToken>,
    var_names: &mut VarNameManager,
) -> Result<(Constraint, OptionalConstraint), ParserError> {
    match lex.next() {
        Some(Ok(OPBToken::Integer)) => parse_opb_constraint_dyn(lex, var_names),
        Some(Ok(OPBToken::GreaterEqual)) => {
            parse_constraint_empty_terms(lex, Comparator::GreaterEqual)
        }
        Some(Ok(OPBToken::Equal)) => parse_constraint_empty_terms(lex, Comparator::Equal),
        _ => Err(ParserError::token_error(
            lex.span(),
            "'*', 'min:', '>=', '<=', or integer",
        )),
    }
}

/// Special case for parsing OPB constraints without terms.
fn parse_constraint_empty_terms(
    lex: &mut Lexer<OPBToken>,
    comparator: Comparator,
) -> Result<(Constraint, OptionalConstraint), ParserError> {
    is_integer(lex)?;
    let degree: BigInt = lex.slice().parse().unwrap();
    check_constraint_end(lex)?;

    match degree.to_i64() {
        Some(degree) => Ok(get_constraints_from_terms(comparator, vec![], 0, degree)),
        None => match degree.to_i128() {
            Some(degree) => Ok(get_constraints_from_terms(comparator, vec![], 0, degree)),
            None => Ok(get_constraints_from_terms(
                comparator,
                vec![],
                BigInt::zero(),
                degree,
            )),
        },
    }
}

/// Helper function to parse a constraint.
///
/// This function iteratively tries to parse the constraint as [`i64`], then [`i128`] and finally as [`BigInt`] to get a constraint that has a small as possible size. The function returns a [`PBConstraintEnum`], which can directly be added to a database.
///
/// **Note:** When this function is called the lexer has already parsed the first token, which should be an integer.
fn parse_opb_constraint_dyn(
    lex: &mut Lexer<OPBToken>,
    var_name_manager: &mut VarNameManager,
) -> Result<(Constraint, OptionalConstraint), ParserError> {
    // Parse terms, try to parse into i128 first.
    let mut terms_i64 = Vec::new();
    let mut coeff_sum_i64: i64 = 0;
    let mut comparator =
        parse_opb_terms(lex, var_name_manager, &mut coeff_sum_i64, &mut terms_i64)?;

    if let Some(comparator) = comparator {
        if let Ok(degree) = lex.slice().parse() {
            check_constraint_end(lex)?;
            return Ok(get_constraints_from_terms(
                comparator,
                terms_i64,
                coeff_sum_i64,
                degree,
            ));
        }
    }

    // Try to parse into i128 next.
    let mut terms_i128 = terms_i64.into_iter().map(|t| t.into()).collect();
    let mut coeff_sum_i128: i128 = coeff_sum_i64.into();
    if comparator.is_none() {
        comparator = parse_opb_terms(lex, var_name_manager, &mut coeff_sum_i128, &mut terms_i128)?;
    }
    if let Some(comparator) = comparator {
        if let Ok(degree) = lex.slice().parse() {
            check_constraint_end(lex)?;
            return Ok(get_constraints_from_terms(
                comparator,
                terms_i128,
                coeff_sum_i128,
                degree,
            ));
        }
    }

    // BigInt always succeeds.
    let mut terms_big: Vec<GeneralPBTerm<BigInt>> =
        terms_i128.into_iter().map(|t| t.into()).collect();
    let mut coeff_sum_big: BigInt = coeff_sum_i128.into();
    let comparator = if let Some(comparator) = comparator {
        comparator
    } else {
        parse_opb_terms(lex, var_name_manager, &mut coeff_sum_big, &mut terms_big)?.unwrap()
    };
    let degree = lex.slice().parse().unwrap();
    check_constraint_end(lex)?;
    Ok(get_constraints_from_terms(
        comparator,
        terms_big,
        coeff_sum_big,
        degree,
    ))
}

/// Helper function to parse terms with generic parameter `N`.
///
/// The function returns an [`Err`] if the tokenization failed. The function returns [`Ok(None)`] if a coefficient is larger than the largest or smaller than the smallest number of the type `N`. Hence, the caller of the function should take care of adjusting the size of the coefficients accordingly.
///
/// **Note:** When this function is called the lexer has already parsed the first token, which should be an integer.
#[inline]
fn parse_opb_terms<N: Int>(
    lex: &mut Lexer<OPBToken>,
    var_name_manager: &mut VarNameManager,
    coeff_sum: &mut N,
    terms: &mut Vec<GeneralPBTerm<N>>,
) -> Result<Option<Comparator>, ParserError> {
    let mut coeff: N = match lex.slice().parse() {
        Ok(integer) => integer,
        Err(_) => return Ok(None),
    };

    loop {
        if coeff.is_negative() {
            let abs = match N::zero().checked_sub(&coeff) {
                Some(abs) => abs,
                None => return Ok(None),
            };
            *coeff_sum = match coeff_sum.checked_add(&abs) {
                Some(sum) => sum,
                None => return Ok(None),
            };
        } else {
            *coeff_sum = match coeff_sum.checked_add(&coeff) {
                Some(sum) => sum,
                None => return Ok(None),
            };
        }
        let lit = match lex.next() {
            Some(Ok(OPBToken::Var)) => {
                Lit::from_var(var_name_manager.add_by_name(lex.slice()), false)
            }
            Some(Ok(OPBToken::Negation)) => {
                if lex.next() == Some(Ok(OPBToken::Var)) {
                    Lit::from_var(var_name_manager.add_by_name(lex.slice()), true)
                } else {
                    return Err(ParserError::token_error(lex.span(), "variable name"));
                }
            }
            _ => {
                return Err(ParserError::token_error(lex.span(), "literal"));
            }
        };
        terms.push(GeneralPBTerm::new(coeff, lit));

        coeff = match lex.next() {
            Some(Ok(OPBToken::GreaterEqual)) => {
                is_integer(lex)?;
                return Ok(Some(Comparator::GreaterEqual));
            }
            Some(Ok(OPBToken::LessEqual)) => {
                is_integer(lex)?;
                return Ok(Some(Comparator::LessEqual));
            }
            Some(Ok(OPBToken::Equal)) => {
                is_integer(lex)?;
                return Ok(Some(Comparator::Equal));
            }
            Some(Ok(OPBToken::Integer)) => match lex.slice().parse() {
                Ok(integer) => integer,
                Err(_) => return Ok(None),
            },
            _ => {
                return Err(ParserError::token_error(
                    lex.span(),
                    "'>=', '<=', or integer",
                ));
            }
        };
    }
}

/// Check if the next token is an integer
///
/// The function returns an [`Err`] if the next token is not an integer.
#[inline]
fn is_integer(lex: &mut Lexer<OPBToken>) -> Result<(), ParserError> {
    if lex.next() != Some(Ok(OPBToken::Integer)) {
        return Err(ParserError::token_error(lex.span(), "integer"));
    }
    Ok(())
}

/// Check if the constraint end as specified with `;` and that no further data is specified after the semicolon.
#[inline]
fn check_constraint_end(lex: &mut Lexer<OPBToken>) -> Result<(), ParserError> {
    if lex.next() != Some(Ok(OPBToken::Semicolon)) {
        Err(ParserError::token_error(lex.span(), "';'"))?;
    }

    Ok(())
}

/// Create a [`PBConstraintEnum`] from `terms` and `degree`.
///
/// This function detects if the PB constraint is a cardinality constraint or clause and returns the most specific type. If the `comparator` used in the pseudo-Boolean constraint is `=`, then the constraint is split into two constraints. The function always returns the `>=` direction as the first entry of the tuple and optionally return the `>=` direction.
#[inline]
fn get_constraints_from_terms<N: Int>(
    comparator: Comparator,
    terms: Vec<GeneralPBTerm<N>>,
    coeff_sum: N,
    degree: N,
) -> (PBConstraintEnum, Option<PBConstraintEnum>)
where
    i64: TryFrom<N>,
    PBConstraintEnum: From<GeneralPBConstraint<N>>,
{
    // Compute leq-constraint
    let leq_constraint = if comparator != Comparator::GreaterEqual {
        let negated_terms = terms
            .iter()
            .map(|term| GeneralPBTerm::new(-term.coeff.clone(), term.lit))
            .collect();

        Some(constraint_from_terms_and_coeff_sum(
            negated_terms,
            -degree.clone(),
            coeff_sum.clone(),
        ))
    } else {
        None
    };

    match comparator {
        Comparator::GreaterEqual => (
            constraint_from_terms_and_coeff_sum(terms, degree, coeff_sum),
            None,
        ),
        Comparator::LessEqual => (leq_constraint.unwrap(), None),
        Comparator::Equal => (
            constraint_from_terms_and_coeff_sum(terms, degree, coeff_sum),
            leq_constraint,
        ),
    }
}
