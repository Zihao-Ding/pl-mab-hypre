use std::rc::Rc;

use logos::Lexer;
use veripb_formula::prelude::DBConstraint;
use veripb_parser::error::ParserError;

use crate::prelude::*;

#[derive(Debug)]
pub struct FormulaCheck {
    size: usize,
}

impl FormulaCheck {
    pub fn new(size: usize) -> Self {
        Self { size }
    }

    pub fn parse(lex: Lexer<RuleToken>) -> Result<Self, ParserError> {
        let size = IntegerToken::parse(&mut lex.morph())? as usize;
        Ok(Self { size })
    }
}

impl Rule for FormulaCheck {
    #[inline]
    fn compute(
        &mut self,
        _context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if database.size() != self.size {
            return Err(CheckingError::formula_size_mismatch(
                database.size(),
                self.size,
            ));
        }
        Ok(vec![])
    }
}
