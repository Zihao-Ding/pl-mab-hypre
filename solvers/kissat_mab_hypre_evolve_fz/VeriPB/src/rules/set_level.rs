use std::rc::Rc;

use logos::Lexer;
use veripb_formula::prelude::*;
use veripb_parser::error::ParserError;

use crate::prelude::*;

#[derive(Debug)]
pub struct SetLevelRule {
    level: usize,
}

impl SetLevelRule {
    #[inline]
    pub fn new(level: usize) -> Self {
        SetLevelRule { level }
    }

    #[inline]
    pub fn parse(lex: Lexer<RuleToken>) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let level = IntegerToken::parse(&mut lex)?;
        if level.is_negative() {
            return Err(ParserError::token_error(lex.span(), "non-negative integer"));
        }
        Ok(SetLevelRule {
            level: level as usize,
        })
    }
}

impl Rule for SetLevelRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if self.level >= context.level_ids.len() {
            context.level_ids.resize(self.level + 1, Default::default());
        }
        context.current_level = Some(self.level);
        Ok(vec![])
    }
}
