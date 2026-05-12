use std::rc::Rc;

use veripb_formula::prelude::DBConstraint;

use crate::prelude::*;

#[derive(Debug)]
pub struct UnimplementedRule {
    #[allow(dead_code)]
    feature: String,
}

impl UnimplementedRule {
    pub fn new(feature: String) -> Self {
        Self { feature }
    }
}

impl Rule for UnimplementedRule {
    fn compute(
        &mut self,
        _context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        Err(CheckingError::not_implemented(&self.feature))
    }

    #[inline]
    fn is_subproof_friendly(&self) -> bool {
        true
    }
}
