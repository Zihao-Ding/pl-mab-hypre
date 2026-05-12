use std::rc::Rc;

use logos::Lexer;
use veripb_formula::prelude::*;
use veripb_parser::{error::ParserError, opb_parser::parse_single_constraint};

use crate::prelude::*;

#[derive(Debug)]
pub struct IsDeletedCheck {
    constraint: Rc<DBConstraint>,
}

impl IsDeletedCheck {
    pub fn new(constraint: PBConstraintEnum) -> Self {
        Self {
            constraint: Rc::new(DBConstraint::from(constraint)),
        }
    }

    pub fn parse(lex: Lexer<RuleToken>, context: &mut Context) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let (geq_constraint, leq_constraint) =
            parse_single_constraint(&mut lex, &mut context.var_names)?;
        if leq_constraint.is_some() {
            Err(ParserError::token_error(
                0..lex.span().end,
                "inequality constraint",
            ))
        } else {
            Ok(IsDeletedCheck {
                constraint: Rc::new(DBConstraint::from(geq_constraint)),
            })
        }
    }
}

impl Rule for IsDeletedCheck {
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        database.update_unique_index(&mut context.propagation_engine)?;
        if database.lookup(&self.constraint).is_some() {
            Err(CheckingError::DeletedConstraintInDB)
        } else {
            Ok(vec![])
        }
    }
}
