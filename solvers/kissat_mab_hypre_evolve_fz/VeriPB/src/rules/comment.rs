use std::rc::Rc;

use veripb_formula::prelude::DBConstraint;

use crate::prelude::*;

#[derive(Debug)]
pub struct Comment;

impl Rule for Comment {
    #[inline]
    fn compute(
        &mut self,
        _context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        Ok(vec![])
    }

    #[inline]
    fn is_subproof_friendly(&self) -> bool {
        true
    }
}
