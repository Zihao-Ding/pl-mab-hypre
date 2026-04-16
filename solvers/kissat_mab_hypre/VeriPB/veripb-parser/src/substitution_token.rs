//! Tokenizer for VeriPB substitutions.

use logos::Logos;

/// VeriPB substitution token.
#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r" |\t|\r|\n|->|→")]
pub enum SubstitutionToken {
    /// Positive literals in OPB format.
    #[regex("[a-zA-Z_][_a-zA-Z0-9\\-\\^\\[\\]\\{\\}]+")]
    PositiveLit,

    /// Negative literals in OPB format.
    #[regex("~[a-zA-Z_][_a-zA-Z0-9\\-\\^\\[\\]\\{\\}]+")]
    NegativeLit,

    /// Constant 0 (false) value in a substitution. I.e., the variable before the `0` is assigned to false.
    #[token("0")]
    Zero,

    /// Constant 1 (true) value in a substitution. I.e., the variable before the `1` is assigned to true.
    #[token("1")]
    One,

    /// Token to finalize the a substitution.
    #[token(";")]
    Semicolon,
}
