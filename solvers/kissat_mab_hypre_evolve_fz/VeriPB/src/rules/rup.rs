use std::rc::Rc;

use colored::Colorize;
use logos::Lexer;
use veripb_formula::prelude::*;
use veripb_parser::{error::ParserError, opb_parser::parse_single_constraint};

use crate::prelude::*;

/// Check that there is a constraint in the database that is equal to the expected constraint.
#[derive(Debug)]
pub struct RUPRule {
    constraint: Rc<DBConstraint>,
    hint: Option<Vec<isize>>,
}

impl RUPRule {
    pub fn new(constraint: PBConstraintEnum, hint: Option<Vec<isize>>) -> Self {
        Self {
            constraint: Rc::new(DBConstraint::from(constraint)),
            hint,
        }
    }

    pub fn parse(lex: Lexer<RuleToken>, context: &mut Context) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let (geq_constraint, leq_constraint) =
            parse_single_constraint(&mut lex, &mut context.var_names)?;
        if leq_constraint.is_some() {
            Err(ParserError::token_error(
                0..lex.span().end,
                "inequality constraint",
            ))
        } else {
            let mut lex = lex.morph();
            match RUPHint::parse_optional(&mut lex)? {
                Some(integer) => {
                    let mut hint = vec![0, integer];
                    while let Some(integer) = RUPHint::parse_optional(&mut lex)? {
                        hint.push(integer);
                    }
                    Ok(RUPRule {
                        constraint: Rc::new(DBConstraint::from(geq_constraint)),
                        hint: Some(hint),
                    })
                }
                None => Ok(RUPRule {
                    constraint: Rc::new(DBConstraint::from(geq_constraint)),
                    hint: None,
                }),
            }
        }
    }

    fn trace_failed_with_hints(
        &self,
        database: &Database,
        context: &mut Context,
        negated: DBConstraint,
    ) {
        context.annotated_rup_assignment.reset();

        println!("Propagation check failed! The propagation had the following trail:");
        println!("  propagations in format: <assignment> (<reason constraint>)");

        loop {
            let mut unchanged = true;
            for id in self.hint.as_ref().unwrap().iter() {
                let id = database.normalize_id(*id);
                if id == 0 {
                    for lit in negated.traced_propagate(&mut context.annotated_rup_assignment) {
                        unchanged = false;
                        println!(
                            "    {} ({})",
                            lit.to_pretty_string(&context.var_names).purple(),
                            negated.to_pretty_string(&context.var_names).cyan(),
                        )
                    }
                } else {
                    let constraint = database.get_entry_usize(id as usize).unwrap();
                    for lit in constraint.traced_propagate(&mut context.annotated_rup_assignment) {
                        unchanged = false;
                        println!(
                            "    {} ({})",
                            lit.to_pretty_string(&context.var_names).purple(),
                            constraint.to_pretty_string(&context.var_names).cyan(),
                        )
                    }
                };
            }
            if unchanged {
                return;
            }
        }
    }
}

impl Rule for RUPRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        let mut proof_buf = None;
        if let Some(elaborator) = context.elaborator.as_mut() {
            proof_buf = Some(&mut elaborator.proof_buf);
        }
        match &self.hint {
            None => {
                // Do unit propagation check to see if constraint is implied.
                database.update_propagation_index(&mut context.propagation_engine)?;
                if context
                    .propagation_engine
                    .reverse_unit_propagation_check(
                        &context.var_names,
                        &[],
                        Some(&self.constraint),
                        context.only_core,
                        &mut proof_buf,
                        context.args.trace_failed,
                    )?
                    .is_conflict()
                {
                    Ok(vec![self.constraint.clone()])
                } else {
                    Err(CheckingError::NotRUP(context.only_core))
                }
            }
            Some(hint) => {
                // Do propagation based on the constraint IDs given as hint.
                context.annotated_rup_assignment.reset();
                context
                    .annotated_rup_assignment
                    .resize(context.var_names.len());
                let negated = self.constraint.negate();

                loop {
                    let mut unchanged = true;
                    for id in hint {
                        let id = database.normalize_id(*id);
                        if id == 0 {
                            match negated.propagate(&mut context.annotated_rup_assignment) {
                                ConstraintPropagationResult::Conflict => {
                                    if let Some(buf) = proof_buf.as_mut() {
                                        buf.push_str(" ~");
                                    }
                                    return Ok(vec![self.constraint.clone()]);
                                }
                                ConstraintPropagationResult::NoPropagation => {}
                                ConstraintPropagationResult::Propagated => {
                                    unchanged = false;
                                    if let Some(buf) = proof_buf.as_mut() {
                                        buf.push_str(" ~");
                                    }
                                }
                            }
                        } else {
                            let constraint = database.get_entry_usize(id as usize)?;
                            if context.only_core && !constraint.is_core_constraint_id(id as usize) {
                                return Err(CheckingError::CoreSubproofUsingNonCoreConstraint(id));
                            }
                            match constraint.propagate(&mut context.annotated_rup_assignment) {
                                ConstraintPropagationResult::Conflict => {
                                    if let Some(buf) = proof_buf.as_mut() {
                                        buf.push(' ');
                                        buf.push_str(
                                            &constraint
                                                .get_out_id(constraint.get_some_id())
                                                .expect("should have output ID")
                                                .to_string(),
                                        );
                                    }
                                    return Ok(vec![self.constraint.clone()]);
                                }
                                ConstraintPropagationResult::NoPropagation => {}
                                ConstraintPropagationResult::Propagated => {
                                    unchanged = false;
                                    if let Some(buf) = proof_buf.as_mut() {
                                        buf.push(' ');
                                        buf.push_str(
                                            &constraint
                                                .get_out_id(constraint.get_some_id())
                                                .expect("should have output ID")
                                                .to_string(),
                                        );
                                    }
                                }
                            }
                        };
                    }
                    if unchanged {
                        if context.args.trace_failed {
                            self.trace_failed_with_hints(database, context, negated);
                        }
                        return Err(CheckingError::NotRUP(context.only_core));
                    }
                }
            }
        }
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.write("rup ");
        elaborator.write(&self.constraint.to_pretty_string(&context.var_names));
        elaborator.write(" :");
        elaborator.write_and_clear_buf();
        elaborator.writeln(";");
        Ok(())
    }

    #[inline]
    fn is_subproof_friendly(&self) -> bool {
        true
    }
}
