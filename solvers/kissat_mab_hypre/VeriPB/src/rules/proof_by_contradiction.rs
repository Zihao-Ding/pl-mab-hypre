use std::rc::Rc;

use veripb_formula::prelude::*;

use crate::prelude::*;

#[derive(Debug)]
pub struct ProofByContradiction {
    constraint: Rc<DBConstraint>,
    has_subproof: bool,
}

impl ProofByContradiction {
    pub fn new(constraint: PBConstraintEnum, has_subproof: bool) -> Self {
        Self {
            constraint: Rc::new(DBConstraint::from(constraint)),
            has_subproof,
        }
    }
}

impl Rule for ProofByContradiction {
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        // Maybe we can quickly autoprove that the constraint is implied.
        if !self.has_subproof {
            // Check if constraint is in DB.
            database.update_unique_index(&mut context.propagation_engine)?;
            if let Some(constraint) = database.lookup(&self.constraint) {
                if !context.only_core || constraint.is_core_constraint() {
                    if context.args.trace {
                        println!("* constraint is in DB already")
                    }
                    if let Some(elaborator) = context.elaborator.as_mut() {
                        elaborator.write("pol ");
                        elaborator.writeln(
                            &constraint
                                .get_out_id(constraint.get_some_id())
                                .unwrap()
                                .to_string(),
                        );
                    }

                    return Ok(vec![self.constraint.clone()]);
                }
            }
        }

        // We need to elaborate the red rule line here, as autoproving might generate extra lines.
        if let Some(elaborator) = context.elaborator.as_mut() {
            elaborator.write("pbc ");
            elaborator.write(&self.constraint.to_pretty_string(&context.var_names));
            elaborator.writeln(" : subproof");
        }

        if context.args.trace {
            println!("  ** contradiction has to be shown in subproof **");
        }

        let negated = Rc::new(self.constraint.negate());
        let mut subproof_context = SubproofContext::new(database.len());
        subproof_context.proof_by_contradiction_subproof = true;

        if self.has_subproof {
            subproof_context.additional_hint = Some(database.len());
            subproof_context.to_add.push(self.constraint.clone());

            context
                .subcontexts
                .push(Subcontext::Subproof(subproof_context));
            Ok(vec![negated])
        } else {
            if let Some(elaborator) = context.elaborator.as_mut() {
                negated.set_out_id(0, elaborator.inc_id());
            }

            subproof_context.finalize(context, database, &[negated])?;
            Ok(vec![self.constraint.clone()])
        }
    }

    #[inline]
    fn is_subproof_friendly(&self) -> bool {
        true
    }
}
