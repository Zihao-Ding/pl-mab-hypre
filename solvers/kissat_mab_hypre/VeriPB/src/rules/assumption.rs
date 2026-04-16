use std::rc::Rc;

use logos::Lexer;
use veripb_formula::prelude::{DBConstraint, PBConstraintEnum};
use veripb_parser::{error::ParserError, opb_parser::parse_single_constraint};

use crate::prelude::*;

#[derive(Debug)]
pub struct AssumptionRule {
    constraint: Rc<DBConstraint>,
}

impl AssumptionRule {
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
            let mut lex = lex.morph();
            match IntegerToken::parse_optional(&mut lex)? {
                Some(integer) => {
                    let mut hint = vec![0, integer];
                    while let Some(integer) = IntegerToken::parse_optional(&mut lex)? {
                        hint.push(integer);
                    }
                    Ok(AssumptionRule {
                        constraint: Rc::new(DBConstraint::from(geq_constraint)),
                    })
                }
                None => Ok(AssumptionRule {
                    constraint: Rc::new(DBConstraint::from(geq_constraint)),
                }),
            }
        }
    }
}

impl Rule for AssumptionRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        context.assumption_used = true;
        Ok(vec![self.constraint.clone()])
    }

    #[inline]
    fn elaborate(
        &self,
        _context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        Err(ElaborationError::AssumptionUsed)
    }

    #[inline]
    fn is_subproof_friendly(&self) -> bool {
        true
    }
}
