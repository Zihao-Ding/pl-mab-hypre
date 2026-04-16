//! Tokenizer for VeriPB assignments.

use logos::Logos;

/// VeriPB assignment token.
///
/// A token in an assignment is either a positive variable or a negative variable.
#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum AssignmentToken {
    /// Positive literal in OPB format.
    #[regex("[a-zA-Z_][_a-zA-Z0-9\\-\\^\\[\\]\\{\\}]+")]
    PositiveVar,

    /// Negative literal in OPB format.
    #[regex("~[a-zA-Z_][_a-zA-Z0-9\\-\\^\\[\\]\\{\\}]+")]
    NegativeVar,
}
