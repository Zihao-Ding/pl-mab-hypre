//! Tokenizer for constraints in OPB format.

use logos::Logos;

/// Tokens used in the OPB format as [specified by the PB competition 2024](https://www.cril.univ-artois.fr/PB24/OPBgeneral.pdf).
#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum OPBToken {
    /// Comment lines.
    #[regex("\\*.*")]
    Comment,

    /// Integer used for coefficients or degree (right-hand side).
    #[regex("[+-]?[0-9]+")]
    Integer,

    /// OPB variable.
    #[regex("[a-zA-Z_][_a-zA-Z0-9\\-\\^\\[\\]\\{\\}]+")]
    Var,

    /// Negation symbol for a literal.
    #[token("~")]
    Negation,

    /// Greater than or equal comparison for PB constraint.
    #[token(">=")]
    GreaterEqual,

    /// Less than or equal comparison for PB constraint.
    #[token("<=")]
    LessEqual,

    /// Equal comparison for PB constraint.
    #[token("=")]
    Equal,

    /// Semicolon which should end a PB constraint.
    #[token(";")]
    Semicolon,

    /// Label to start the objective function that should be minimized.
    #[token("min:")]
    Minimize,

    /// Label to start the objective function that should be maximized. While we support maximization objectives in the OPB file, internally the objective is always minimized.
    #[token("max:")]
    Maximize,

    /// Constraint labels similar to the ones used in the proof format.
    #[regex("@[a-zA-Z0-9_^\\[\\]\\{\\}]+")]
    Label,
}

#[cfg(test)]
mod test {
    use logos::Logos;

    use crate::opb_token::OPBToken;

    #[test]
    fn integer() {
        let mut lex = OPBToken::lexer("424 -424 +424 +004");

        assert_eq!(lex.next(), Some(Ok(OPBToken::Integer)));
        assert_eq!(lex.next(), Some(Ok(OPBToken::Integer)));
        assert_eq!(lex.next(), Some(Ok(OPBToken::Integer)));
        assert_eq!(lex.next(), Some(Ok(OPBToken::Integer)));
        assert_eq!(lex.slice().parse::<i64>().unwrap(), 4);
        assert_eq!(lex.next(), None);
    }

    #[test]
    fn variable() {
        let mut lex = OPBToken::lexer("x12 _x12 xaK2-[]{}_^");

        assert_eq!(lex.next(), Some(Ok(OPBToken::Var)));
        assert_eq!(lex.next(), Some(Ok(OPBToken::Var)));
        assert_eq!(lex.next(), Some(Ok(OPBToken::Var)));
        assert_eq!(lex.next(), None);
    }

    #[test]
    fn comment() {
        let mut lex = OPBToken::lexer("*sdf sdafsd ffsdf sdf asdf dsfsdf 12 sda f\n*\n11");

        assert_eq!(lex.next(), Some(Ok(OPBToken::Comment)));
        assert_eq!(lex.next(), Some(Ok(OPBToken::Comment)));
        assert_eq!(lex.next(), Some(Ok(OPBToken::Integer)));
        assert_eq!(lex.next(), None);
    }
}
