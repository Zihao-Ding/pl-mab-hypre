use std::rc::Rc;

use veripb_formula::prelude::DBConstraint;

use crate::prelude::*;

use super::Rule;

const PROOF_END: &str = "end pseudo-Boolean proof;";

#[derive(Debug)]
pub struct EndProof;

impl Rule for EndProof {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if context.has_end_proof {
            return Err(CheckingError::DoubleEnd);
        }
        if !context.has_output || !context.has_conclusion {
            return Err(CheckingError::WrongFooterOrder(PROOF_END));
        }

        context.has_end_proof = true;
        Ok(vec![])
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        context.elaborator.as_mut().unwrap().writeln(PROOF_END);
        Ok(())
    }
}
