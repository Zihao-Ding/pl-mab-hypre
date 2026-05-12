use std::fmt::Debug;

use logos::{Lexer, Logos};
use veripb_formula::prelude::*;
use veripb_parser::error::ParserError;

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum IntegerToken {
    // Integer used for coefficients or degree (right-hand side).
    #[regex(r"[+-]?\d+")]
    Integer,
}

impl IntegerToken {
    pub fn parse_optional(lex: &mut Lexer<IntegerToken>) -> Result<Option<isize>, ParserError> {
        match lex.next() {
            Some(Ok(_)) => Ok(Some(lex.slice().parse()?)),
            Some(Err(_)) => Err(ParserError::token_error(lex.span(), "constraint ID")),
            None => Ok(None),
        }
    }

    pub fn parse(lex: &mut Lexer<IntegerToken>) -> Result<isize, ParserError> {
        match lex.next() {
            Some(Ok(_)) => Ok(lex.slice().parse()?),
            _ => Err(ParserError::token_error(lex.span(), "integer")),
        }
    }
}

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum RUPHint {
    // Integer used for coefficients or degree (right-hand side).
    #[regex(r"[+-]?\d+")]
    Integer,

    // The tilde symbol represents the negated constraint of a RUP step.
    #[token("~")]
    Tilde,
}

impl RUPHint {
    pub fn parse_optional(lex: &mut Lexer<RUPHint>) -> Result<Option<isize>, ParserError> {
        match lex.next() {
            Some(Ok(RUPHint::Integer)) => Ok(Some(lex.slice().parse()?)),
            Some(Ok(RUPHint::Tilde)) => Ok(Some(0)),
            Some(Err(_)) => Err(ParserError::token_error(lex.span(), "constraint ID")),
            None => Ok(None),
        }
    }

    pub fn parse(lex: &mut Lexer<RUPHint>) -> Result<isize, ParserError> {
        match lex.next() {
            Some(Ok(RUPHint::Integer)) => Ok(lex.slice().parse()?),
            Some(Ok(RUPHint::Tilde)) => Ok(0),
            _ => Err(ParserError::token_error(lex.span(), "integer")),
        }
    }
}

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum IntegerOrSemicolonToken {
    // Integer used for coefficients or degree (right-hand side).
    #[regex(r"[+-]?\d+")]
    Integer,

    // Semicolons
    #[token(";")]
    Semicolon,
}

impl IntegerOrSemicolonToken {
    pub fn parse(lex: &mut Lexer<Self>) -> Result<Option<isize>, ParserError> {
        match lex.next() {
            Some(Ok(Self::Integer)) => Ok(Some(lex.slice().parse().unwrap())),
            Some(Ok(Self::Semicolon)) => Ok(None),
            Some(Err(_)) => Err(ParserError::token_error(lex.span(), "constraint ID")),
            None => Ok(None),
        }
    }
}

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum IdentifierOption {
    // Get constraints by a list of IDs.
    #[token("id")]
    Id,

    // Get constraints in between two constraint ID including the first ID and not including the second ID.
    #[token("range")]
    Range,

    // Get the constraint specified in OPB format.
    #[token("spec")]
    #[token("find")]
    Specification,
}

impl IdentifierOption {
    pub fn parse(lex: &mut Lexer<Self>) -> Result<Self, ParserError> {
        match lex.next() {
            Some(Ok(option)) => Ok(option),
            _ => Err(ParserError::token_error(
                lex.span(),
                "'id', 'range', 'spec', or 'find'",
            )),
        }
    }
}

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum SubproofBeginToken {
    #[token("begin")]
    Begin,
}

impl SubproofBeginToken {
    pub fn parsed_begin(lex: &mut Lexer<Self>) -> Result<bool, ParserError> {
        match lex.next() {
            Some(Ok(Self::Begin)) => Ok(true),
            None => Ok(false),
            _ => Err(ParserError::token_error(
                lex.span(),
                "'begin' or end of line",
            )),
        }
    }
}

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum OrderName {
    #[regex(r"[a-zA-Z][_a-zA-Z0-9\\-\\^\\[\\]\\{\\}]+")]
    Name,
}

impl OrderName {
    #[inline]
    pub fn parse(lex: &mut Lexer<Self>) -> Result<String, ParserError> {
        match lex.next() {
            Some(Ok(Self::Name)) => Ok(lex.slice().to_string()),
            _ => Err(ParserError::token_error(lex.span(), "a name for the order")),
        }
    }

    #[inline]
    pub fn parse_optional(lex: &mut Lexer<Self>) -> Result<Option<String>, ParserError> {
        match lex.next() {
            Some(Ok(Self::Name)) => Ok(Some(lex.slice().to_string())),
            None => Ok(None),
            _ => Err(ParserError::token_error(
                lex.span(),
                "a name for the order or nothing",
            )),
        }
    }
}

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum VariableToken {
    #[regex(r"[_a-zA-Z][_a-zA-Z0-9\\-\\^\\[\\]\\{\\}]+")]
    Variable,
}

impl VariableToken {
    #[inline]
    pub fn parse_optional(
        lex: &mut Lexer<Self>,
        var_names: &mut VarNameManager,
    ) -> Result<Option<VarIdx>, ParserError> {
        match lex.next() {
            Some(Ok(Self::Variable)) => Ok(Some(var_names.add_by_name(lex.slice()))),
            None => Ok(None),
            _ => Err(ParserError::token_error(lex.span(), "variable")),
        }
    }
}

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum LiteralToken {
    // OPB variable name.
    #[regex("[a-zA-Z_][_a-zA-Z0-9\\-\\^\\[\\]\\{\\}]+")]
    Variable,

    // Negation symbol for a literal.
    #[regex("~[a-zA-Z_][_a-zA-Z0-9\\-\\^\\[\\]\\{\\}]+")]
    NegatedVariable,
}

impl LiteralToken {
    #[inline]
    pub fn parse_optional(
        lex: &mut Lexer<Self>,
        var_names: &mut VarNameManager,
    ) -> Result<Option<Lit>, ParserError> {
        match lex.next() {
            Some(Ok(Self::Variable)) => {
                let var = var_names.add_by_name(lex.slice());
                Ok(Some(Lit::from_var(var, false)))
            }
            Some(Ok(Self::NegatedVariable)) => {
                let var = var_names.add_by_name(&lex.slice()[1..]);
                Ok(Some(Lit::from_var(var, true)))
            }
            None => Ok(None),
            _ => Err(ParserError::token_error(lex.span(), "literal")),
        }
    }
}
