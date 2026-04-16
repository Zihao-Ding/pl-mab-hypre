use std::rc::Rc;

use logos::Lexer;
use veripb_formula::prelude::*;
use veripb_parser::error::ParserError;

use crate::prelude::*;

#[derive(Debug)]
pub struct ProofgoalRule {
    proofgoal_id: ProofgoalID,
}

impl ProofgoalRule {
    pub fn new(proofgoal_id: ProofgoalID) -> Self {
        Self { proofgoal_id }
    }

    pub fn parse(lex: Lexer<RuleToken>) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let proofgoal_id = match lex.next() {
            Some(Ok(proofgoal_id)) => proofgoal_id,
            _ => return Err(ParserError::token_error(lex.span(), "proofgoal identifier")),
        };

        Ok(Self { proofgoal_id })
    }
}

impl Rule for ProofgoalRule {
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if let Some(Subcontext::Subproof(subproof_context)) = context.subcontexts.last_mut() {
            if subproof_context.proofgoal_subproof {
                return Err(CheckingError::StartProofgoalWhileInsideProofgoal);
            }
            let proofgoal = match &mut self.proofgoal_id {
                ProofgoalID::Internal(index) => subproof_context.pop_internal_goal(*index)?,
                ProofgoalID::Database(index) => {
                    *index = database.normalize_id(*index);
                    subproof_context.pop_database_goal(*index as usize, database)?
                }
            };
            let constraints = proofgoal.into_counterexample();
            let proofgoal_context =
                Subcontext::Subproof(SubproofContext::new_proofgoal_subproof(database.len()));
            context.subcontexts.push(proofgoal_context);
            Ok(constraints)
        } else {
            Err(CheckingError::ProofgoalOutsideSubproof)
        }
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        database: &Database,
    ) -> Result<(), ElaborationError> {
        if let Some(Subcontext::Subproof(subproof_context)) =
            context.subcontexts.get(context.subcontexts.len() - 2)
        {
            if subproof_context.proof_by_contradiction_subproof {
                context.elaborator.as_mut().unwrap().dec_id();
                let hint = subproof_context.additional_hint.unwrap();
                database
                    .entries
                    .last()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .set_out_id(
                        database.len() - 1,
                        database
                            .get_entry_usize(hint)
                            .expect("Proof by contradiction should have additional hint")
                            .get_out_id(hint)
                            .expect("Constraint should have output ID."),
                    );
                return Ok(());
            }
        }
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.write("\tproofgoal ");
        match self.proofgoal_id {
            ProofgoalID::Internal(id) => {
                elaborator.write("#");
                elaborator.writeln(&id.to_string());
            }
            ProofgoalID::Database(index) => {
                let constraint = database
                    .get_entry_usize(index as usize)
                    .expect("constraint already accessed before");
                elaborator.writeln(
                    &constraint
                        .get_out_id(index as usize)
                        .expect("constraint should have output ID")
                        .to_string(),
                );
            }
        }
        Ok(())
    }

    #[inline]
    fn is_subproof_friendly(&self) -> bool {
        true
    }
}
