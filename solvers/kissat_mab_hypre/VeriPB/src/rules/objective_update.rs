use std::{fmt::Display, rc::Rc};

use logos::{Lexer, Logos};
use malachite_bigint::BigInt;
use veripb_formula::{pb_constraint::constraint_from_terms_and_coeff_sum, prelude::*};
use veripb_parser::{error::ParserError, opb_parser::parse_opb_objective};

use crate::prelude::*;
use num_traits::{Signed, Zero};

// use super::ScopeId;

// Specification of how the objective is update.
#[derive(Debug, Logos, PartialEq, Eq, Default, Copy, Clone)]
#[logos(skip r"[ \t\r\n]")]
pub enum ObjectiveUpdateType {
    // Use specified objective as new objective.
    #[token("new")]
    #[default]
    New,

    // Use specified objective as objective difference.
    #[token("diff")]
    Diff,
}

impl ObjectiveUpdateType {
    pub fn parse(lex: &mut Lexer<Self>) -> Result<Self, ParserError> {
        match lex.next() {
            Some(Ok(option)) => Ok(option),
            _ => Err(ParserError::token_error(lex.span(), "'new', or 'diff'")),
        }
    }
}

impl Display for ObjectiveUpdateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectiveUpdateType::New => write!(f, "new"),
            ObjectiveUpdateType::Diff => write!(f, "diff"),
        }
    }
}

#[derive(Debug)]
pub struct ObjectiveUpdateRule {
    objective: PBObjective,
    update_type: ObjectiveUpdateType,
    has_subproof: bool,
}

impl ObjectiveUpdateRule {
    pub fn new(
        objective: PBObjective,
        update_type: ObjectiveUpdateType,
        has_subproof: bool,
    ) -> Self {
        Self {
            objective,
            update_type,
            has_subproof,
        }
    }

    pub fn parse(lex: Lexer<RuleToken>, context: &mut Context) -> Result<Self, ParserError> {
        // Parse objective update type.
        let mut lex = lex.morph();
        let update_type = ObjectiveUpdateType::parse(&mut lex)?;

        // Parse objective.
        let mut lex = lex.morph();
        let objective = parse_opb_objective(&mut lex, &mut context.var_names, false)?;

        // Parse optional subproof begin.
        let has_subproof = SubproofBeginToken::parsed_begin(&mut lex.morph())?;

        Ok(Self {
            objective,
            update_type,
            has_subproof,
        })
    }

    #[inline]
    fn get_proofgoals(&self, context: &Context) -> (Rc<DBConstraint>, Rc<DBConstraint>) {
        match self.update_type {
            ObjectiveUpdateType::New => {
                // Compute f_new >= f_old first and for f_old >= f_new the degree and the coefficients are just negated.
                let old_objective = context.objective.as_ref().unwrap();
                let new_geq_old_degree = old_objective.constant.clone() - &self.objective.constant;
                let old_geq_new_degree = -new_geq_old_degree.clone();
                let mut new_geq_old_terms =
                    self.objective.terms.values().cloned().collect::<Vec<_>>();
                new_geq_old_terms.extend(
                    old_objective
                        .terms
                        .values()
                        .map(|term| GeneralPBTerm::new(-term.coeff.clone(), term.lit)),
                );
                let old_geq_new_terms = new_geq_old_terms
                    .iter()
                    .map(|term| GeneralPBTerm::new(-term.coeff.clone(), term.lit))
                    .collect::<Vec<_>>();
                let coeff_sum = new_geq_old_terms
                    .iter()
                    .fold(Zero::zero(), |sum: BigInt, term| sum + &term.coeff.abs());
                let new_geq_old_constraint = constraint_from_terms_and_coeff_sum(
                    new_geq_old_terms,
                    new_geq_old_degree,
                    coeff_sum.clone(),
                );
                let old_geq_new_constraint = constraint_from_terms_and_coeff_sum(
                    old_geq_new_terms,
                    old_geq_new_degree,
                    coeff_sum,
                );

                (
                    Rc::new(new_geq_old_constraint.into()),
                    Rc::new(old_geq_new_constraint.into()),
                )
            }
            ObjectiveUpdateType::Diff => {
                let coeff_abs_sum = self
                    .objective
                    .terms
                    .values()
                    .fold(Zero::zero(), |sum: BigInt, term| sum + &term.coeff.abs());

                // The proofgoal f_new >= f_old is just `diff_terms >= -diff_constant`.
                let new_geq_old_constraint = constraint_from_terms_and_coeff_sum(
                    self.objective.terms.values().cloned().collect(),
                    -self.objective.constant.clone(),
                    coeff_abs_sum.clone(),
                );

                // The proofgoal f_old >= f_new is just `-diff_terms >= diff_constant`.
                let old_geq_new_constraint = constraint_from_terms_and_coeff_sum(
                    self.objective
                        .terms
                        .values()
                        .map(|term| GeneralPBTerm::new(-term.coeff.clone(), term.lit))
                        .collect(),
                    self.objective.constant.clone(),
                    coeff_abs_sum,
                );

                (
                    Rc::new(new_geq_old_constraint.into()),
                    Rc::new(old_geq_new_constraint.into()),
                )
            }
        }
    }
}

impl Rule for ObjectiveUpdateRule {
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if context.objective.is_none() {
            return Err(CheckingError::ObjectiveUpdateNoObjective);
        }

        // We need to elaborate the obju rule line here, as autoproving might generate extra lines.
        if let Some(elaborator) = context.elaborator.as_mut() {
            elaborator.write("obju ");
            elaborator.write(&self.update_type.to_string());
            elaborator.write(" ");
            elaborator.write(&self.objective.to_pretty_string(&context.var_names));
            elaborator.writeln(" : subproof");
        }

        // Set up subproof subcontext.
        let mut subproof_context = SubproofContext::new(database.len());
        context.only_core = true;

        let (new_geq_old_constraint, old_geq_new_constraint) = self.get_proofgoals(context);

        subproof_context.add_single_proofgoal(new_geq_old_constraint, None);
        if context.args.trace {
            println!("  ** proofgoal from new objective >= old objective **");
            subproof_context.trace_internal_proofgoal_back(&context.var_names);
        }

        subproof_context.add_single_proofgoal(old_geq_new_constraint, None);
        if context.args.trace {
            println!("  ** proofgoal from old objective >= new objective **");
            subproof_context.trace_internal_proofgoal_back(&context.var_names);
        }

        // Start subproof and change to new objective.
        if self.has_subproof {
            subproof_context.objective_update = Some(std::mem::take(&mut self.objective));
            subproof_context.objective_update_type = self.update_type;
            context
                .subcontexts
                .push(Subcontext::Subproof(subproof_context));
        } else {
            subproof_context.finalize(context, database, &[])?;
            context.update_objective(std::mem::take(&mut self.objective), self.update_type);
            context.only_core = false;
        }

        Ok(vec![])
    }
}
