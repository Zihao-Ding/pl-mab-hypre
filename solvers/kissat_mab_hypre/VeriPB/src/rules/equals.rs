use std::rc::Rc;

use logos::Lexer;
use veripb_formula::prelude::{DBConstraint, PBConstraintEnum};
use veripb_parser::{error::ParserError, opb_parser::parse_single_constraint};

use crate::prelude::*;

/// Check that there is a constraint in the database that is equal to the expected constraint.
#[derive(Debug)]
pub struct EqualsRule {
    constraint: Rc<DBConstraint>,
    hint: Option<isize>,
    add_result: bool,
}

impl EqualsRule {
    pub fn new(constraint: PBConstraintEnum, hint: Option<isize>, add_result: bool) -> Self {
        Self {
            constraint: Rc::new(DBConstraint::from(constraint)),
            hint,
            add_result,
        }
    }

    #[inline]
    pub fn parse(
        lex: Lexer<super::RuleToken>,
        context: &mut Context,
        add_result: bool,
    ) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let (geq_constraint, leq_constraint) =
            parse_single_constraint(&mut lex, &mut context.var_names)?;
        if leq_constraint.is_some() {
            return Err(ParserError::token_error(
                0..lex.span().end,
                "inequality constraint",
            ));
        }
        let hint = IntegerToken::parse_optional(&mut lex.morph())?;
        Ok(EqualsRule {
            constraint: Rc::new(DBConstraint::from(geq_constraint)),
            hint,
            add_result,
        })
    }
}

impl Rule for EqualsRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        match self.hint.as_mut() {
            None => {
                database.update_unique_index(&mut context.propagation_engine)?;
                if let Some(constraint) = database.lookup(&self.constraint) {
                    if !context.only_core || constraint.is_core_constraint() {
                        self.hint = Some(constraint.get_some_id() as isize);
                    } else {
                        return Err(CheckingError::not_found(&self.constraint, true));
                    }
                } else {
                    return Err(CheckingError::not_found(&self.constraint, false));
                }
            }
            Some(index) => {
                *index = database.normalize_id(*index);
                let constraint = database.get_entry_usize(*index as usize)?;
                if context.only_core && !constraint.is_core_constraint_id(*index as usize) {
                    return Err(CheckingError::CoreSubproofUsingNonCoreConstraint(*index));
                }
                if *constraint != self.constraint {
                    return Err(CheckingError::not_equal(&self.constraint, constraint));
                }
            }
        }
        if self.add_result {
            Ok(vec![self.constraint.clone()])
        } else {
            Ok(vec![])
        }
    }

    #[inline]
    fn get_returned_id(&self) -> Option<isize> {
        self.hint
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        database: &Database,
    ) -> Result<(), ElaborationError> {
        if self.add_result {
            let elaborator = context.elaborator.as_mut().unwrap();
            elaborator.write("pol ");
            let id = self.hint.unwrap();
            let hint_constraint = database
                .get_entry_usize(id as usize)
                .expect("constraint already accessed before");
            elaborator.write(
                &hint_constraint
                    .get_out_id(id as usize)
                    .expect("constraint should have out ID")
                    .to_string(),
            );
            elaborator.writeln(";");
        }

        Ok(())
    }

    #[inline]
    fn is_subproof_friendly(&self) -> bool {
        true
    }
}
