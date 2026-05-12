use std::rc::Rc;

use logos::Lexer;
use malachite_bigint::BigInt;
use num_traits::Zero;
use veripb_formula::{
    pb_constraint::{constraint_from_terms, constraint_from_terms_and_coeff_sum},
    prelude::*,
    var_type::VarType,
};
use veripb_parser::{assignment_parser::parse_bool_assignment_to_raw_vec, error::ParserError};

use crate::prelude::*;

#[derive(Debug, PartialEq)]
pub enum SolutionRuleOutput {
    None,
    Improving,
    Excluding,
}

#[derive(Debug)]
pub struct SolutionRule {
    solution: Vec<Lit>,
    propagated_solution: Assignment<BooleanVar>,
    rule_output: SolutionRuleOutput,
    objective_value: Option<BigInt>,
}

impl SolutionRule {
    pub fn new(
        solution: Vec<Lit>,
        rule_output: SolutionRuleOutput,
        objective_value: Option<BigInt>,
    ) -> Self {
        Self {
            solution,
            propagated_solution: Assignment::default(),
            rule_output,
            objective_value,
        }
    }

    pub fn parse(
        lex: Lexer<RuleToken>,
        context: &mut Context,
        rule_output: SolutionRuleOutput,
    ) -> Result<Self, ParserError> {
        let solution = parse_bool_assignment_to_raw_vec(&mut lex.morph(), &mut context.var_names)?;

        Ok(SolutionRule {
            solution,
            rule_output,
            propagated_solution: Assignment::default(),
            objective_value: None,
        })
    }
}

impl Rule for SolutionRule {
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if self.rule_output == SolutionRuleOutput::Improving && context.objective.is_none() {
            return Err(CheckingError::SolutionImprovingRequiresObjective);
        }

        let assignment = check_solution(context, database, &self.solution)?;

        // Keep track of currently best solution.
        let value = if let Some(objective) = &context.objective {
            objective
                .evaluate(&assignment)
                .ok_or(CheckingError::ObjectiveUnassigned)?
        } else {
            BigInt::zero()
        };

        // Check if claimed objective value matches computed objective value.
        if context.objective.is_none() && self.objective_value.is_some() {
            return Err(CheckingError::SolutionValueHintNoObjective);
        }
        if self
            .objective_value
            .as_ref()
            .is_some_and(|claimed_value| *claimed_value != value)
        {
            return Err(CheckingError::SolutionIncorrectValue(
                self.objective_value.clone().unwrap(),
                value,
            ));
        }
        if context
            .best_objective_value
            .as_ref()
            .is_none_or(|best_value| value < *best_value)
        {
            context.best_objective_value = Some(value.clone());
            if context.args.checked_deletion {
                context.best_valid_objective_value = Some(value.clone())
            }
        }

        // Store propagated solution for potential elaboration.
        self.propagated_solution = assignment;

        // Return constraint according to the output type of the rule.
        match self.rule_output {
            SolutionRuleOutput::None => Ok(vec![]),
            SolutionRuleOutput::Excluding => {
                if context.args.show_warnings {
                    println!(
                        "Warning: The `solx` rule is currently work in progress and its behaviour might change in the future."
                    );
                }
                let mut terms = Vec::new();
                for (var_idx, value) in self.propagated_solution.assignment.iter().enumerate() {
                    match value.get_value() {
                        BoolValue::Unassigned => {}
                        BoolValue::Assigned(true) => {
                            terms.push(GeneralPBTerm::new(1, Lit::from_var(var_idx, true)));
                        }
                        BoolValue::Assigned(false) => {
                            terms.push(GeneralPBTerm::new(1, Lit::from_var(var_idx, false)));
                        }
                    }
                }
                let coeff_sum = terms.len() as i64;
                let constraint = constraint_from_terms_and_coeff_sum(terms, 1_i64, coeff_sum);
                Ok(vec![Rc::new(DBConstraint::from(constraint))])
            }
            SolutionRuleOutput::Improving => {
                let mut negated_terms: Vec<_> = context
                    .objective
                    .as_ref()
                    .unwrap()
                    .terms
                    .values()
                    .cloned()
                    .collect();
                for term in negated_terms.iter_mut() {
                    term.coeff = -term.coeff.clone();
                }

                let constraint = constraint_from_terms(
                    negated_terms,
                    BigInt::from(1) + &context.objective.as_ref().unwrap().constant - &value,
                )
                .into_smallest_type();
                Ok(vec![Rc::new(DBConstraint::from(constraint))])
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
        match self.rule_output {
            SolutionRuleOutput::None => elaborator.write("sol "),
            SolutionRuleOutput::Improving => elaborator.write("soli "),
            SolutionRuleOutput::Excluding => return Err(ElaborationError::SolxNotInKernel),
        }
        elaborator.write(
            &self
                .propagated_solution
                .to_pretty_string(&context.var_names),
        );
        if self.objective_value.is_some() {
            elaborator.write(" : ");
            elaborator.write(&self.objective_value.clone().unwrap().to_string());
        }
        elaborator.writeln(";");
        Ok(())
    }

    #[inline]
    fn add_constraints_to_core(&self, _context: &Context) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct ObjectiveValueRule {
    value: BigInt,
}

impl ObjectiveValueRule {
    pub fn new(value: BigInt) -> Self {
        Self { value }
    }
}

impl Rule for ObjectiveValueRule {
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if context.objective.is_none() {
            return Err(CheckingError::ObjectiveImprovingRequiresObjective);
        }
        if context
            .best_objective_value
            .as_ref()
            .is_none_or(|best_value| self.value < *best_value)
        {
            context.best_objective_value = Some(self.value.clone());
        }
        let mut negated_terms: Vec<_> = context
            .objective
            .as_ref()
            .unwrap()
            .terms
            .values()
            .cloned()
            .collect();
        for term in negated_terms.iter_mut() {
            term.coeff = -term.coeff.clone();
        }

        let constraint = constraint_from_terms(
            negated_terms,
            BigInt::from(1) + &context.objective.as_ref().unwrap().constant - &self.value,
        )
        .into_smallest_type();
        Ok(vec![Rc::new(DBConstraint::from(constraint))])
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.write("obji ");
        elaborator.write(&self.value.to_string());
        elaborator.writeln(";");
        Ok(())
    }

    #[inline]
    fn add_constraints_to_core(&self, _context: &Context) -> bool {
        true
    }
}
