//! Parsing functions for DIMACS CNF format.
//!
//! The DIMACS CNF format is used to represent SAT formulas in CNF. It was first used in the 2nd DIMACS implementation challenge.
//!
//! Tokenization is performed using [`CNFToken`].

use logos::{Lexer, Logos};
use std::path::Path;
use veripb_formula::prelude::*;

use crate::{cnf_token::CNFToken, error::ParserError, parser::get_lines};

/// Parse a formula from a DIMACS CNF file.
pub fn parse_cnf_from_file<P>(filename: P) -> Result<(Formula, usize), ParserError>
where
    P: AsRef<Path>,
{
    let mut database = Formula::default();
    let mut num_vars = 0;
    let lines = get_lines(&filename)?;
    let mut parsed_header = false;
    let mut current_lits = Vec::new();

    for (line_number, line) in lines.map_while(Result::ok).enumerate() {
        let mut lex = CNFToken::lexer(&line);

        if parsed_header {
            while let Some(token) = lex.next() {
                match token {
                    Ok(CNFToken::Integer(Ok(integer))) => match integer {
                        0 => {
                            let clause =
                                Clause::from_unnormalized_lits(current_lits.clone()).into();
                            database.constraints.push(clause);
                            current_lits.clear();
                        }
                        ..0 => {
                            current_lits.push(Lit::from_var((-integer) as usize, true));
                        }
                        1.. => {
                            current_lits.push(Lit::from_var((integer) as usize, false));
                        }
                    },
                    Ok(CNFToken::Integer(Err(_))) => {
                        return Err(ParserError::token_error_with_file(
                            lex.span(),
                            "64 bit integer",
                            filename.as_ref().to_string_lossy().to_string(),
                            line_number,
                        ))
                    }
                    Ok(CNFToken::Comment) => {}
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
        } else {
            match lex.next() {
                Some(Ok(CNFToken::ProblemHeader)) => {
                    let header = parse_header(&mut lex).map_err(|e| {
                        e.add_file_and_line(
                            filename.as_ref().to_string_lossy().to_string(),
                            line_number,
                        )
                        .unwrap()
                    })?;
                    num_vars = header.0;
                    database.reserve_exact(header.1);
                    parsed_header = true;
                }
                Some(Ok(CNFToken::Comment)) => {}
                _ => {
                    return Err(ParserError::token_error_with_file(
                        lex.span(),
                        "CNF header or comment",
                        filename.as_ref().to_string_lossy().to_string(),
                        line_number,
                    ));
                }
            }
        }
    }

    if parsed_header {
        Ok((database, num_vars))
    } else {
        Err(ParserError::NoHeader)
    }
}

/// Parse the problem header of an DIMACS CNF file.
#[inline]
fn parse_header(lex: &mut Lexer<CNFToken>) -> Result<(usize, usize), ParserError> {
    if let Some(Ok(CNFToken::Integer(Ok(num_vars)))) = lex.next() {
        if let Some(Ok(CNFToken::Integer(Ok(num_clauses)))) = lex.next() {
            return Ok((num_vars as usize, num_clauses as usize));
        }
    }

    Err(ParserError::token_error(lex.span(), "integer in isize"))
}
