//! Parsing functions for VeriPB assignments.
//!
//! A VeriPB assignment is a list of OPB literals. For instance:
//! ```ignore
//! x1 ~x13 x42 ~x21 ~variable
//! ```
//! sets variables `x1` and `x42` to true and `x13`, `x21`, and `variable` to false.
//!
//! Tokenization is preformed using the [`AssignmentToken`].

use std::io::{Error, ErrorKind};

use logos::Lexer;
use veripb_formula::prelude::*;

use crate::assignment_token::AssignmentToken;

/// Parse an assignment from a lexer to an [`Assignment`] of [`BooleanVar`]. This function returns a new [`Assignment`].
///
/// See [`parse_bool_assignment_into()`] for more details.
pub fn parse_bool_assignment(
    lex: &mut Lexer<AssignmentToken>,
    var_names: &mut VarNameManager,
) -> Result<Assignment<BooleanVar>, Error> {
    let mut assignment = Assignment::with_size(var_names.len());

    parse_bool_assignment_into(lex, var_names, &mut assignment)?;

    Ok(assignment)
}

/// Parse an assignment from a lexer into an existing [`Assignment`] of [`BooleanVar`].
///
/// The assignment is formatted as a list of literals. A positive literal represents an assignment of the variable to true and a negative literal represents an assignment of the variable to false.
///
/// If a variable is already assigned in `assignment`, then it will be overwritten by the parsed assignment.
pub fn parse_bool_assignment_into(
    lex: &mut Lexer<AssignmentToken>,
    var_names: &mut VarNameManager,
    assignment: &mut Assignment<BooleanVar>,
) -> Result<(), Error> {
    assignment.resize(var_names.len());

    while let Some(lit) = lex.next() {
        match lit {
            Ok(AssignmentToken::PositiveVar) => {
                let var = var_names.add_by_name(lex.slice());
                if var >= assignment.len() {
                    assignment.resize(var + 1);
                }
                assignment.set_value(var, BoolValue::Assigned(true));
            }
            Ok(AssignmentToken::NegativeVar) => {
                let var = var_names.add_by_name(&lex.slice()[1..]);
                if var >= assignment.len() {
                    assignment.resize(var + 1);
                }
                assignment.set_value(var, BoolValue::Assigned(false));
            }
            Err(_) => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("The token '{}' is not a literal!", lex.slice()),
                ));
            }
        }
    }

    Ok(())
}

/// Parse an assignment from a lexer to a raw [`Vec<Lit>`].
///
/// This vector can be used to initialize an assignment or a propagation trail.
pub fn parse_bool_assignment_to_raw_vec(
    lex: &mut Lexer<AssignmentToken>,
    var_names: &mut VarNameManager,
) -> Result<Vec<Lit>, Error> {
    let mut assignment = Vec::new();

    while let Some(lit) = lex.next() {
        match lit {
            Ok(AssignmentToken::PositiveVar) => {
                let var = var_names.add_by_name(lex.slice());
                assignment.push(Lit::from_var(var, false));
            }
            Ok(AssignmentToken::NegativeVar) => {
                let var = var_names.add_by_name(&lex.slice()[1..]);
                assignment.push(Lit::from_var(var, true));
            }
            Err(_) => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("The token '{}' is not a literal!", lex.slice()),
                ));
            }
        }
    }

    Ok(assignment)
}
