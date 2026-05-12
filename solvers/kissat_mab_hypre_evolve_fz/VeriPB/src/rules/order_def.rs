use std::rc::Rc;

use veripb_formula::prelude::*;

use crate::prelude::*;

#[derive(Debug)]
pub struct OrderDefRule;

impl Rule for OrderDefRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if let Some(Subcontext::Order(order_context)) = context.subcontexts.last_mut() {
            order_context.inside_def = true;
            Ok(vec![])
        } else {
            Err(CheckingError::DefNotAllowedHere)
        }
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        context.elaborator.as_mut().unwrap().writeln("\tdef");
        Ok(())
    }
}
