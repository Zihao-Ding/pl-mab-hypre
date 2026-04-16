use std::rc::Rc;

use veripb_formula::prelude::*;

use crate::{order_context::TransitivityContext, prelude::*};

#[derive(Debug)]
pub struct OrderTransitivityRule;

impl Rule for OrderTransitivityRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        context
            .subcontexts
            .push(Subcontext::Transitivity(TransitivityContext::default()));
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
            .writeln("\ttransitivity");
        Ok(())
    }
}
