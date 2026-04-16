use std::rc::Rc;

use logos::{Lexer, Logos};
use veripb_formula::prelude::*;
use veripb_parser::{error::ParserError, opb_parser::parse_single_constraint, opb_token::OPBToken};

use crate::prelude::*;

use super::ConstraintDefRuleToken;

#[derive(Debug)]
pub struct OrderDefConstraintRule {
    constraint: Rc<DBConstraint>,
}

impl OrderDefConstraintRule {
    pub fn new(constraint: PBConstraintEnum) -> Self {
        Self {
            constraint: Rc::new(DBConstraint::from(constraint)),
        }
    }

    pub fn parse(
        lex: Lexer<ConstraintDefRuleToken>,
        context: &mut Context,
    ) -> Result<Self, ParserError> {
        let mut lex = OPBToken::lexer(lex.source());
        let (geq_constraint, leq_constraint) =
            parse_single_constraint(&mut lex, &mut context.var_names)?;
        if leq_constraint.is_some() {
            Err(ParserError::token_error(
                0..lex.span().end,
                "inequality constraint",
            ))
        } else {
            Ok(OrderDefConstraintRule {
                constraint: Rc::new(DBConstraint::from(geq_constraint)),
            })
        }
    }
}

impl Rule for OrderDefConstraintRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if let Some(Subcontext::Order(order_context)) = context.subcontexts.last_mut() {
            let contains_only_order_vars = match &self.constraint.constraint {
                PBConstraintEnum::Clause(clause) => clause.get_lits().all(|lit| {
                    order_context
                        .constructed_order
                        .is_order_variable(lit.get_var())
                }),
                PBConstraintEnum::Cardinality(cardinality) => cardinality.get_lits().all(|lit| {
                    order_context
                        .constructed_order
                        .is_order_variable(lit.get_var())
                }),
                PBConstraintEnum::GeneralPBI64(constraint) => constraint.get_lits().all(|lit| {
                    order_context
                        .constructed_order
                        .is_order_variable(lit.get_var())
                }),
                PBConstraintEnum::GeneralPBI128(constraint) => constraint.get_lits().all(|lit| {
                    order_context
                        .constructed_order
                        .is_order_variable(lit.get_var())
                }),
                PBConstraintEnum::GeneralPBBigInt(constraint) => constraint.get_lits().all(|lit| {
                    order_context
                        .constructed_order
                        .is_order_variable(lit.get_var())
                }),
            };
            if contains_only_order_vars {
                order_context
                    .constructed_order
                    .definition
                    .push(self.constraint.clone());
                Ok(vec![])
            } else {
                Err(CheckingError::UndeclaredVariablesInOrder)
            }
        } else {
            Err(CheckingError::OnlyAllowedInOrderDef)
        }
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.write("\t\t");
        elaborator.write(&self.constraint.to_pretty_string(&context.var_names));
        elaborator.writeln(";");
        Ok(())
    }
}
