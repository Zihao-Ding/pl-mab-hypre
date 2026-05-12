use std::rc::Rc;

use veripb_formula::prelude::*;

use crate::{order_context::ReflexivityContext, prelude::*};

#[derive(Debug)]
pub struct OrderReflexivityRule;

impl Rule for OrderReflexivityRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        context
            .subcontexts
            .push(Subcontext::Reflexivity(ReflexivityContext::default()));
        Ok(vec![])
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        context
            .elaborator
            .as_mut()
            .unwrap()
            .writeln("\treflexivity");
        Ok(())
    }
}
