use std::rc::Rc;

use veripb_formula::prelude::*;

use crate::prelude::*;

#[derive(Debug)]
pub struct OrderVariablesRule;

impl Rule for OrderVariablesRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        match context.subcontexts.last_mut() {
            Some(Subcontext::Order(order_context)) => {
                order_context.inside_vars = true;
                Ok(vec![])
            }
            Some(Subcontext::Transitivity(transitivity_context)) => {
                transitivity_context.inside_vars = true;
                Ok(vec![])
            }
            _ => Err(CheckingError::VarsNotAllowedHere),
        }
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        context.elaborator.as_mut().unwrap().writeln("\tvars");
        Ok(())
    }
}
