use std::rc::Rc;

use veripb_formula::prelude::DBConstraint;

use crate::prelude::*;

#[derive(Debug)]
pub struct FailRule;

impl Rule for FailRule {
    fn compute(
        &mut self,
        _context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        Err(CheckingError::FailProofUsed)
    }
}
