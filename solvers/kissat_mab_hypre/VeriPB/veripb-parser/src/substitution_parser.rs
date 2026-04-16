//! Parsing functions for VeriPB substitutions.
//!
//! A substitution in VeriPB format is a list of pairs of variables and literals or truth values. For instance:
//! ```ignore
//! x1 1 x12 0 variable x1 another ~x1
//! ```
//! maps `x1` to true, `x12` to false, `variable` to `x1`, and `another` to not `x1`.
//!
//! Optionally, it is possible to add the arrow `->` inside a pair, so that our example becomes:
//! ```ignore
//! x1 -> 1 x12 -> 0 variable -> x1 another -> ~x1
//! ```
//!
//! Tokenization is performed by using [`SubstitutionToken`].

use std::io::{Error, ErrorKind};

use logos::Lexer;
use veripb_formula::prelude::*;

use crate::substitution_token::SubstitutionToken;

/// Parse a substitution in VeriPB format into the [`Substitution`] data structure.
///
/// The substitution is parsed from a lexer that generates tokens of the substitution. A format of a substitution is a list of mappings. A the domain of the mapping are variables and the range of the mapping is `0`, `1`, or a literal.
pub fn parse_substitution(
    lex: &mut Lexer<SubstitutionToken>,
    var_names: &mut VarNameManager,
) -> Result<Substitution, Error> {
    let mut sub = Substitution::default();
    while let Some(token) = lex.next() {
        // Parse the domain variable of the substitution map.
        let var = match token {
            Ok(SubstitutionToken::PositiveLit) => var_names.add_by_name(lex.slice()),
            Ok(SubstitutionToken::Semicolon) => break,
            Ok(_) => return Err(Error::new(ErrorKind::InvalidData, "Expected variable.")),
            Err(_) => return Err(Error::new(ErrorKind::InvalidData, "Unrecognized token.")),
        };

        // Parse the image of the parsed variable.
        if let Some(token) = lex.next() {
            let already_set = match token {
                Ok(SubstitutionToken::Zero) => sub.set(var, SubstitutionValue::FALSE),
                Ok(SubstitutionToken::One) => sub.set(var, SubstitutionValue::TRUE),
                Ok(SubstitutionToken::PositiveLit) => sub.set(
                    var,
                    SubstitutionValue::lit(Lit::from_var(
                        var_names.add_by_name(lex.slice()),
                        false,
                    )),
                ),
                Ok(SubstitutionToken::NegativeLit) => sub.set(
                    var,
                    SubstitutionValue::lit(Lit::from_var(
                        var_names.add_by_name(&lex.slice()[1..]),
                        true,
                    )),
                ),
                Ok(_) => {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "Expected '0', '1', or literal.",
                    ))
                }
                Err(_) => return Err(Error::new(ErrorKind::InvalidData, "Unrecognized token.")),
            };
            if already_set {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "A variable is assigned twice in the substitution.",
                ));
            }
        } else {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Substitution ended unexpectedly after variable.",
            ));
        }
    }

    Ok(sub)
}
