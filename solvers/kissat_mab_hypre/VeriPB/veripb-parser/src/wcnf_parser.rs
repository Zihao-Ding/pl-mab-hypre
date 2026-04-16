//! Parsing functions for WCNF MaxSAT files.
//!
//! The format has been specified in the [MaxSAT Evaluation 2022](https://maxsat-evaluations.github.io/2022/rules.html#input).

use logos::Logos;
use malachite_bigint::BigInt;
use num_traits::Zero;
use std::path::Path;
use veripb_formula::prelude::*;

use crate::{error::ParserError, parser::get_lines, wcnf_token::WCNFToken};

enum Weight {
    Hard,
    Soft(BigInt),
}

/// Parse a formula from a WCNF file. The WCNF format is described in the [MaxSAT Evaluation 2022](https://maxsat-evaluations.github.io/2022/rules.html#input).
///
/// This function creates a [`VarNameManager`] and calls the function [`parse_wcnf_from_file_given_var_manager()`]. See [`parse_wcnf_from_file_given_var_manager()`] for more details.
#[inline]
pub fn parse_wcnf_from_file<P>(filename: P) -> Result<(Formula, VarNameManager), ParserError>
where
    P: AsRef<Path>,
{
    let mut var_name_manager = VarNameManager::default();
    let formula = parse_wcnf_from_file_given_var_manager(filename, &mut var_name_manager)?;
    Ok((formula, var_name_manager))
}

/// Parse a formula from a WCNF file. The WCNF format is described in the [MaxSAT Evaluation 2022](https://maxsat-evaluations.github.io/2022/rules.html#input).
///
/// For converting MaxSAT problem into a pseudo-Boolean minimization problem, we use the following conventions:
/// - Hard clauses are constraints.
/// - For empty soft clauses, increase the objective constant.
/// - Unit soft clauses are added immediately to the objective with the weight as coefficient and the negated clause literal as the objective literal.
/// - For any other soft clauses `C`, introduce a relaxation variable `_b<i>`, where `<i>` means that this clause is the ith clause. Then `~_b<i>` is added with the weight as coefficient to the objective and the clause `C or ~_b<i>` is added as a constraint.
pub fn parse_wcnf_from_file_given_var_manager<P>(
    filename: P,
    var_name_manager: &mut VarNameManager,
) -> Result<Formula, ParserError>
where
    P: AsRef<Path>,
{
    let mut database = Formula::default();
    let lines = get_lines(&filename)?;
    let mut current_lits = Vec::new();
    let mut weight = None;
    let mut objective_terms = Vec::new();
    let mut objective_constant = BigInt::zero();
    let mut clause_counter: u64 = 0;

    for (line_number, line) in lines.map_while(Result::ok).enumerate() {
        let mut lex = WCNFToken::lexer(&line);

        while let Some(token) = lex.next() {
            match token {
                Ok(WCNFToken::Integer) => {
                    if weight.is_none() {
                        weight = Some(Weight::Soft(lex.slice().parse().unwrap()));
                    } else {
                        let integer: i64 = lex.slice().parse::<i64>().map_err(|_| {
                            ParserError::token_error_with_file(
                                lex.span(),
                                "literal",
                                filename.as_ref().to_string_lossy().to_string(),
                                line_number,
                            )
                        })?;
                        match integer {
                            0 => {
                                clause_counter += 1;
                                match std::mem::take(&mut weight) {
                                    None => {
                                        return Err(ParserError::token_error_with_file(
                                            lex.span(),
                                            "weight",
                                            filename.as_ref().to_string_lossy().to_string(),
                                            line_number,
                                        ))
                                    }
                                    Some(Weight::Hard) => {
                                        let clause =
                                            Clause::from_unnormalized_lits(current_lits.clone())
                                                .into();
                                        database.constraints.push(clause);
                                    }
                                    Some(Weight::Soft(coeff)) => {
                                        match current_lits.len() {
                                            0 => {
                                                objective_constant += coeff;
                                            }
                                            1 => {
                                                // Unit soft clauses are added directly to the objective.
                                                let mut unit_lit = current_lits.pop().unwrap();
                                                unit_lit.negate();
                                                objective_terms
                                                    .push(GeneralPBTerm::new(coeff, unit_lit));
                                            }
                                            _ => {
                                                let blocking_lit = Lit::from_var(
                                                    var_name_manager.add_by_name(
                                                        &(String::from("_b")
                                                            + (clause_counter)
                                                                .to_string()
                                                                .as_str()),
                                                    ),
                                                    true,
                                                );
                                                current_lits.push(blocking_lit);
                                                let clause = Clause::from_unnormalized_lits(
                                                    current_lits.clone(),
                                                )
                                                .into();
                                                database.constraints.push(clause);
                                                objective_terms
                                                    .push(GeneralPBTerm::new(coeff, blocking_lit));
                                            }
                                        }
                                    }
                                }
                                current_lits.clear();
                            }
                            ..0 => {
                                current_lits.push(Lit::from_var(
                                    var_name_manager
                                        .add_by_name(&(String::from("x") + &lex.slice()[1..])),
                                    true,
                                ));
                            }
                            1.. => {
                                current_lits.push(Lit::from_var(
                                    var_name_manager
                                        .add_by_name(&(String::from("x") + lex.slice())),
                                    false,
                                ));
                            }
                        }
                    }
                }
                Ok(WCNFToken::HardClause) => {
                    if weight.is_some() {
                        return Err(ParserError::token_error_with_file(
                            lex.span(),
                            "previous clause has to end before next weight",
                            filename.as_ref().to_string_lossy().to_string(),
                            line_number,
                        ));
                    }
                    weight = Some(Weight::Hard);
                }
                Ok(WCNFToken::Comment) => {}
                _ => {
                    return Err(ParserError::token_error_with_file(
                        lex.span(),
                        "integer or comment",
                        filename.as_ref().to_string_lossy().to_string(),
                        line_number,
                    ));
                }
            }
        }
    }

    database.objective = Some(PBObjective::from_terms(
        objective_terms,
        objective_constant,
        false,
    ));

    Ok(database)
}
