use std::rc::Rc;

use logos::Lexer;
use veripb_formula::prelude::DBConstraint;
use veripb_parser::error::ParserError;

use crate::prelude::*;

#[derive(Debug)]
pub struct HeaderRule;

impl HeaderRule {
    #[inline]
    pub fn parse(lex: Lexer<RuleToken>, context: &mut Context) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        context.major_version = Some(IntegerToken::parse(&mut lex)? as u8);
        // Skip the dot separating major and minor version.
        lex.bump(1);
        context.minor_version = Some(IntegerToken::parse(&mut lex)? as u8);

        Ok(HeaderRule)
    }
}

impl Rule for HeaderRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if context.major_version.unwrap() < 2 {
            return Err(CheckingError::UnsupportedProofVersion);
        }
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use logos::Logos;
    use veripb_formula::prelude::VarNameManager;

    use crate::{args::Args, context::Context, rules::RuleToken};

    use super::HeaderRule;

    #[test]
    fn parse_version() {
        let lex = RuleToken::lexer("2.1");
        let mut context = Context::new(Args::default(), VarNameManager::default());

        assert_eq!(context.major_version, None);
        assert_eq!(context.minor_version, None);

        HeaderRule::parse(lex, &mut context).unwrap();

        assert_eq!(context.major_version, Some(2));
        assert_eq!(context.minor_version, Some(1));
    }
}
