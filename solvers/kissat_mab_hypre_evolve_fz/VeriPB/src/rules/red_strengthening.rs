use std::{num::Saturating, rc::Rc};

use logos::Lexer;
use veripb_formula::prelude::*;
use veripb_parser::{
    error::ParserError, opb_parser::parse_single_constraint,
    substitution_parser::parse_substitution,
};

use crate::{context::REQUIRED_RUP_STREAK, prelude::*};

use super::ScopeId;

#[derive(Debug)]
pub struct RedundanceBasedStrengtheningRule {
    constraint: Rc<DBConstraint>,
    witness: Substitution,
    has_subproof: bool,
    add_to_core: bool,
}

impl RedundanceBasedStrengtheningRule {
    pub fn new(constraint: PBConstraintEnum, witness: Substitution, has_subproof: bool) -> Self {
        Self {
            constraint: Rc::new(DBConstraint::from(constraint)),
            witness,
            has_subproof,
            add_to_core: false,
        }
    }

    pub fn parse(lex: Lexer<RuleToken>, context: &mut Context) -> Result<Self, ParserError> {
        // Parse constraint.
        let mut lex = lex.morph();
        let (geq_constraint, leq_constraint) =
            parse_single_constraint(&mut lex, &mut context.var_names)?;
        if leq_constraint.is_some() {
            return Err(ParserError::token_error(
                0..lex.span().end,
                "inequality constraint",
            ));
        }

        // Parse witness.
        let mut lex = lex.morph();
        let witness = parse_substitution(&mut lex, &mut context.var_names)?;

        // Parse optional subproof begin.
        let has_subproof = SubproofBeginToken::parsed_begin(&mut lex.morph())?;

        Ok(Self {
            constraint: Rc::new(geq_constraint.into()),
            witness,
            has_subproof,
            add_to_core: false,
        })
    }
}

impl Rule for RedundanceBasedStrengtheningRule {
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        let subcontexts = context.subcontexts.as_slice().last_chunk::<2usize>();
        if let Some([Subcontext::Order(order_context), Subcontext::Specification(_)]) = subcontexts
        {
            let contains_variable_out_of_order = match &self.constraint.constraint {
                PBConstraintEnum::Clause(clause) => clause.get_lits().any(|lit| {
                    !order_context
                        .constructed_order
                        .is_order_variable(lit.get_var())
                }),
                PBConstraintEnum::Cardinality(cardinality) => cardinality.get_lits().any(|lit| {
                    !order_context
                        .constructed_order
                        .is_order_variable(lit.get_var())
                }),
                PBConstraintEnum::GeneralPBI64(constraint) => constraint.get_lits().any(|lit| {
                    !order_context
                        .constructed_order
                        .is_order_variable(lit.get_var())
                }),
                PBConstraintEnum::GeneralPBI128(constraint) => constraint.get_lits().any(|lit| {
                    !order_context
                        .constructed_order
                        .is_order_variable(lit.get_var())
                }),
                PBConstraintEnum::GeneralPBBigInt(constraint) => constraint.get_lits().any(|lit| {
                    !order_context
                        .constructed_order
                        .is_order_variable(lit.get_var())
                }),
            };
            if contains_variable_out_of_order {
                Err(CheckingError::UndeclaredVariablesInOrder)?
            } else if self
                .witness
                .syntactic_support()
                .any(|var| !order_context.constructed_order.is_auxiliary_variable(*var))
            {
                Err(CheckingError::SpecWitnessMapsNonAuxVariable)?
            }
        }

        // Maybe the strengthening statements in the proof are actually just RUP constraints. We only do this check if we had enough success autoproving the subproof at top-level without using the witness.
        if !self.has_subproof && context.rup_streak >= REQUIRED_RUP_STREAK {
            let mut proof_buf = None;
            if let Some(elaborator) = context.elaborator.as_mut() {
                proof_buf = Some(&mut elaborator.proof_buf)
            }
            // Check if constraint is RUP.
            database.update_propagation_index(&mut context.propagation_engine)?;
            if context
                .propagation_engine
                .reverse_unit_propagation_check(
                    &context.var_names,
                    &[],
                    Some(&self.constraint),
                    context.only_core,
                    &mut proof_buf,
                    false,
                )?
                .is_conflict()
            {
                if context.args.trace {
                    println!("* constraint is RUP *")
                }
                if let Some(elaborator) = context.elaborator.as_mut() {
                    elaborator.write("rup ");
                    elaborator.write(&self.constraint.to_pretty_string(&context.var_names));
                    elaborator.write(" :");
                    elaborator.write_and_clear_buf();
                    elaborator.writeln(";");
                }
                return Ok(vec![self.constraint.clone()]);
            }
            // Check if constraint is in DB.
            database.update_unique_index(&mut context.propagation_engine)?;
            if let Some(constraint) = database.lookup(&self.constraint) {
                if !context.only_core || constraint.is_core_constraint() {
                    if context.args.trace {
                        println!("* constraint is in DB already")
                    }
                    if let Some(elaborator) = context.elaborator.as_mut() {
                        elaborator.write("pol ");
                        elaborator.write(
                            &constraint
                                .get_out_id(constraint.get_some_id())
                                .unwrap()
                                .to_string(),
                        );
                        elaborator.writeln(";");
                    }

                    return Ok(vec![self.constraint.clone()]);
                }
            }
            context.rup_streak = Saturating(0);
        }

        let mut subproof_context = SubproofContext::new(database.len());

        // We need to elaborate the red rule line here, as autoproving might generate extra lines. In version 2, redundance-based strengthening with an empty witness is treated as proof by contradiction.
        if context.major_version <= Some(2) && self.witness.is_empty() {
            subproof_context.proof_by_contradiction_subproof = true;
            if let Some(elaborator) = context.elaborator.as_mut() {
                elaborator.write("pbc ");
                elaborator.write(&self.constraint.to_pretty_string(&context.var_names));
                elaborator.writeln(" : subproof");
            }
        } else {
            self.add_to_core = context.is_strengthening_to_core;
            if let Some(elaborator) = context.elaborator.as_mut() {
                elaborator.write("red ");
                elaborator.write(&self.constraint.to_pretty_string(&context.var_names));
                elaborator.write(" :");
                elaborator.write(&self.witness.to_pretty_string(&context.var_names));
                elaborator.writeln(" : subproof");
            }
        }

        // Generate the proofgoals, starting with the constraint to be introduced.
        subproof_context.add_single_proofgoal(
            Rc::new(self.constraint.substitute(&self.witness)),
            Some(ScopeId::LessEqual),
        );
        if context.args.trace {
            println!("  ** proofgoal from satisfying added constraint **");
            subproof_context.trace_internal_proofgoal_back(&context.var_names);
        }

        // Proofgoals from order.
        if let Some(active_order) = &mut context.active_order {
            for proofgoal in active_order.get_proofgoals(&self.witness) {
                subproof_context.add_internal_proofgoal(proofgoal);
            }
            if context.args.trace {
                println!("  ** proofgoals from order **");
                for id in 2..subproof_context.internal_proofgoal_len() {
                    subproof_context.trace_internal_proofgoal(id, &context.var_names);
                }
            }
        }

        // Proofgoal from the objective.
        if let Some(objective) = &context.objective {
            subproof_context.add_single_proofgoal(
                objective.get_proofgoal(&self.witness),
                Some(ScopeId::LessEqual),
            );
            if context.args.trace {
                println!("  ** proofgoal from objective **");
                subproof_context.trace_internal_proofgoal_back(&context.var_names);
            }
        }

        // Get database proofgoals.
        database.update_unique_index(&mut context.propagation_engine)?;
        for constraint in database.get_proofgoals(
            &self.witness,
            !context.is_strengthening_to_core,
            context.elaborator.is_some(),
            context.only_core,
        ) {
            subproof_context.add_database_proofgoal(
                constraint,
                !context.is_strengthening_to_core,
                Some(ScopeId::LessEqual),
            );
        }
        if context.args.trace {
            subproof_context.trace_database_proofgoals(&context.var_names);
        }

        let negated = Rc::new(self.constraint.negate());

        if self.has_subproof {
            subproof_context.additional_hint = Some(database.len());
            subproof_context.to_add.push(self.constraint.clone());
            context.inside_strengthening_subproof = true;
            subproof_context.witness = std::mem::take(&mut self.witness);
            subproof_context.add_to_core = self.add_to_core;
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
        self.witness.is_empty()
    }

    #[inline]
    fn add_constraints_to_core(&self, _context: &Context) -> bool {
        self.add_to_core
    }
}
