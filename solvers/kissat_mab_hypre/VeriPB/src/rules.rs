mod assumption;
mod comment;
mod conclusion;
mod core;
mod cutting_planes;
mod define_order;
mod deletion;
mod dom_strengthening;
mod end_proof;
mod end_subproof;
mod equal_objective;
mod equals;
mod fail;
mod formula;
mod header;
mod implication;
mod is_deleted;
mod load_order;
mod objective_update;
mod order_aux_vars;
mod order_def;
mod order_def_constraint;
mod order_fresh_aux1;
mod order_fresh_aux2;
mod order_fresh_right_vars;
mod order_left_vars;
mod order_proof;
mod order_reflexivity;
mod order_right_vars;
mod order_specification;
mod order_transitivity;
mod order_vars;
mod output;
mod proof_by_contradiction;
mod proofgoal;
mod red_strengthening;
mod rup;
mod scope;
mod set_level;
mod solution_logging;
mod strengthening_to_core;
mod unimplemented;

use colored::Colorize;
use logos::{Lexer, Logos};
use veripb_formula::prelude::DBConstraint;
use veripb_parser::error::ParserError;
use veripb_propagator::propagation_engine::PropagationEngine;

pub use crate::rules::{
    assumption::AssumptionRule,
    comment::Comment,
    conclusion::{Bound, ConclusionResult, ConclusionRule},
    core::MoveToCoreRule,
    cutting_planes::{Instruction, PolRule},
    define_order::DefineOrderRule,
    deletion::{Deletion, DeletionOption, DeletionOrigin},
    dom_strengthening::DominanceBasedStrengtheningRule,
    end_proof::EndProof,
    end_subproof::EndSubproof,
    equal_objective::EqualsObjectiveRule,
    equals::EqualsRule,
    fail::FailRule,
    formula::FormulaCheck,
    header::HeaderRule,
    implication::ImpliesRule,
    is_deleted::IsDeletedCheck,
    load_order::LoadOrderRule,
    objective_update::{ObjectiveUpdateRule, ObjectiveUpdateType},
    order_aux_vars::OrderAuxVariablesRule,
    order_def::OrderDefRule,
    order_def_constraint::OrderDefConstraintRule,
    order_fresh_aux1::OrderFreshAux1Rule,
    order_fresh_aux2::OrderFreshAux2Rule,
    order_fresh_right_vars::OrderFreshRightVariablesRule,
    order_left_vars::OrderLeftVariablesRule,
    order_proof::OrderProofRule,
    order_reflexivity::OrderReflexivityRule,
    order_right_vars::OrderRightVariablesRule,
    order_specification::OrderSpecificationRule,
    order_transitivity::OrderTransitivityRule,
    order_vars::OrderVariablesRule,
    output::{OutputGuarantee, OutputRule, OutputType},
    proof_by_contradiction::ProofByContradiction,
    proofgoal::ProofgoalRule,
    red_strengthening::RedundanceBasedStrengtheningRule,
    rup::RUPRule,
    scope::{ScopeId, ScopeRule},
    set_level::SetLevelRule,
    solution_logging::{ObjectiveValueRule, SolutionRule, SolutionRuleOutput},
    strengthening_to_core::StrengtheningToCoreRule,
    unimplemented::UnimplementedRule,
};

use crate::{
    context::{Context, Subcontext},
    database::Database,
    deletion_sequence::DeletionSequenceEnum,
    elaborator::ElaborationError,
    error::CheckingError,
};

use std::{fmt::Debug, rc::Rc};

pub trait Rule: Debug {
    #[inline]
    fn trace_rule(&self, lex: Lexer<RuleToken>) {
        println!("{}{}", lex.slice().red(), lex.remainder());
    }

    /// Check the conditions for the rule and return the derived constraints.
    fn compute(
        &mut self,
        _context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError>;

    /// Return the ids of constraints deleted by the rule.
    #[inline]
    fn get_deleted<'a>(
        &'a self,
        _database: &Database,
    ) -> Result<Option<DeletionSequenceEnum<'a>>, CheckingError> {
        Ok(None)
    }

    #[inline]
    fn get_returned_id(&self) -> Option<isize> {
        None
    }

    /// Elaborate the proof rule to the kernel proof.
    #[inline]
    fn elaborate(
        &self,
        _context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        Ok(())
    }

    /// Returns `true` if the rule can be used within a subproof.
    ///
    /// By default, this function returns `false`. To change this behaviour, manually implement this function.
    #[inline]
    fn is_subproof_friendly(&self) -> bool {
        false
    }

    /// Return `true` if resulting constraints are added to the core set instead of the derived set by default.
    #[inline]
    fn add_constraints_to_core(&self, _context: &Context) -> bool {
        false
    }

    /// Returns `true` if the rule swaps out the currently considered database.
    #[inline]
    fn swaps_database(&self) -> bool {
        false
    }

    /// Get the new database to be swapped out.
    fn get_new_database(&mut self) -> (Database, PropagationEngine) {
        unreachable!()
    }

    /// Give the old database to the rule to handle it.
    fn handle_old_database(
        &mut self,
        _context: &mut Context,
        _database: Database,
        _prop_engine: PropagationEngine,
    ) {
        unreachable!()
    }
}

/// Token for the rules.
#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum RuleToken {
    // Header of the proof.
    #[token("pseudo-Boolean proof version")]
    Header,

    #[regex("\\*.*")]
    Comment,

    #[token("pol")]
    #[token("p")]
    PolRule,

    #[token("rup")]
    #[token("u")]
    RUPRule,

    #[token("a")]
    AssumptionRule,

    #[token("red")]
    RedRule,

    #[token("dom")]
    DomRule,

    #[token("core")]
    CoreRule,

    #[token("del")]
    DelRule,

    #[token("d")]
    SingleCharDelRule,

    #[token("delc")]
    DelFromCoreRule,

    #[token("deld")]
    DelFromDerivedRule,

    #[token("is_deleted")]
    IsDeletedCheck,

    #[token("f")]
    CheckFormula,

    #[token("obju")]
    ObjuRule,

    #[token("e")]
    EqualsRule,

    #[token("ea")]
    EqualsAddRule,

    #[token("i")]
    ImpliesRule,

    #[token("ia")]
    ImpliesAddRule,

    #[token("sol")]
    SolRule,

    #[token("soli")]
    #[token("o")]
    SoliRule,

    #[token("solx")]
    #[token("v")]
    SolxRule,

    #[token("conclusion")]
    ConclusionRule,

    #[token("output")]
    OutputRule,

    #[token("def_order")]
    #[token("pre_order")]
    DefineOrder,

    #[token("vars")]
    OrderVariables,

    #[token("left")]
    OrderLeftVariables,

    #[token("right")]
    OrderRightVariables,

    #[token("aux")]
    OrderAuxVariables,

    #[token("def")]
    OrderDef,

    #[token("transitivity")]
    OrderTransitivity,

    #[token("reflexivity")]
    OrderReflexivity,

    #[token("proof")]
    OrderBeginProof,

    #[token("fresh_right")]
    OrderFreshRightVars,

    #[token("fresh_aux_1")]
    OrderFreshAux1,

    #[token("fresh_aux_2")]
    OrderFreshAux2,

    #[token("spec")]
    OrderSpecification,

    #[token("load_order")]
    LoadOrder,

    #[token("end")]
    #[token("qed")]
    EndSubproof,

    #[token("proofgoal")]
    Proofgoal,

    #[token("end pseudo-Boolean proof")]
    EndProof,

    #[token("#")]
    SetLevel,

    #[token("w")]
    WipeLevel,

    #[token("eobj")]
    EqualsObjective,

    #[token("fail")]
    FailProof,

    #[token("strengthening_to_core")]
    StrengtheningToCore,
}

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum ConstraintDefRuleToken {
    #[regex("\\*.*")]
    Comment,

    #[token("end")]
    End,
}

/// Parse a line in the deriviation file into a [`Rule`].
#[inline]
pub fn line_to_rule(
    mut lex: Lexer<RuleToken>,
    context: &mut Context,
) -> Result<Box<dyn Rule>, ParserError> {
    if let Some(Subcontext::Order(order_context)) = context.subcontexts.last() {
        if order_context.inside_def {
            let mut lex = lex.morph();
            return match lex.next() {
                None => Ok(Box::new(Comment)),
                Some(Ok(ConstraintDefRuleToken::Comment)) => Ok(Box::new(Comment)),
                Some(Ok(ConstraintDefRuleToken::End)) => {
                    Ok(Box::new(EndSubproof::parse(lex.morph())?))
                }
                Some(Err(_)) => Ok(Box::new(OrderDefConstraintRule::parse(lex, context)?)),
            };
        }
    }
    match lex.next() {
        None => Ok(Box::new(Comment)),
        Some(Ok(RuleToken::Comment)) => Ok(Box::new(Comment)),
        Some(Ok(RuleToken::Header)) => Ok(Box::new(HeaderRule::parse(lex, context)?)),
        Some(Ok(RuleToken::CheckFormula)) => Ok(Box::new(FormulaCheck::parse(lex)?)),
        Some(Ok(RuleToken::Proofgoal)) => Ok(Box::new(ProofgoalRule::parse(lex)?)),
        Some(Ok(RuleToken::EndSubproof)) => Ok(Box::new(EndSubproof::parse(lex)?)),
        Some(Ok(RuleToken::DelRule)) => Ok(Box::new(Deletion::parse(lex, context)?)),
        Some(Ok(RuleToken::CoreRule)) => Ok(Box::new(MoveToCoreRule::parse(lex)?)),
        Some(Ok(RuleToken::OutputRule)) => Ok(Box::new(OutputRule::parse(lex)?)),
        Some(Ok(RuleToken::ConclusionRule)) => Ok(Box::new(ConclusionRule::parse(lex, context)?)),
        Some(Ok(RuleToken::EqualsRule)) => Ok(Box::new(EqualsRule::parse(lex, context, false)?)),
        Some(Ok(RuleToken::EqualsAddRule)) => Ok(Box::new(EqualsRule::parse(lex, context, true)?)),
        Some(Ok(RuleToken::EndProof)) => Ok(Box::new(EndProof)),
        Some(Ok(RuleToken::RedRule)) => Ok(Box::new(RedundanceBasedStrengtheningRule::parse(
            lex, context,
        )?)),
        Some(Ok(RuleToken::PolRule)) => Ok(Box::new(PolRule::parse(lex, context)?)),
        Some(Ok(RuleToken::ImpliesRule)) => Ok(Box::new(ImpliesRule::parse(lex, context, false)?)),
        Some(Ok(RuleToken::ImpliesAddRule)) => {
            Ok(Box::new(ImpliesRule::parse(lex, context, true)?))
        }
        Some(Ok(RuleToken::RUPRule)) => Ok(Box::new(RUPRule::parse(lex, context)?)),
        Some(Ok(RuleToken::AssumptionRule)) => Ok(Box::new(AssumptionRule::parse(lex, context)?)),
        Some(Ok(RuleToken::SolRule)) => Ok(Box::new(SolutionRule::parse(
            lex,
            context,
            SolutionRuleOutput::None,
        )?)),
        Some(Ok(RuleToken::SolxRule)) => Ok(Box::new(SolutionRule::parse(
            lex,
            context,
            SolutionRuleOutput::Excluding,
        )?)),
        Some(Ok(RuleToken::SoliRule)) => Ok(Box::new(SolutionRule::parse(
            lex,
            context,
            SolutionRuleOutput::Improving,
        )?)),
        Some(Ok(RuleToken::SingleCharDelRule)) => Ok(Box::new(Deletion::parse_ids(
            lex,
            context,
            DeletionOrigin::Unknown,
        )?)),
        Some(Ok(RuleToken::DelFromCoreRule)) => Ok(Box::new(Deletion::parse_ids(
            lex,
            context,
            DeletionOrigin::Core,
        )?)),
        Some(Ok(RuleToken::DelFromDerivedRule)) => Ok(Box::new(Deletion::parse_ids(
            lex,
            context,
            DeletionOrigin::Derived,
        )?)),
        Some(Ok(RuleToken::IsDeletedCheck)) => Ok(Box::new(IsDeletedCheck::parse(lex, context)?)),
        Some(Ok(RuleToken::DomRule)) => Ok(Box::new(DominanceBasedStrengtheningRule::parse(
            lex, context,
        )?)),
        Some(Ok(RuleToken::DefineOrder)) => Ok(Box::new(DefineOrderRule::parse(lex)?)),
        Some(Ok(RuleToken::OrderVariables)) => Ok(Box::new(OrderVariablesRule)),
        Some(Ok(RuleToken::OrderLeftVariables)) => {
            Ok(Box::new(OrderLeftVariablesRule::parse(lex, context)?))
        }
        Some(Ok(RuleToken::OrderRightVariables)) => {
            Ok(Box::new(OrderRightVariablesRule::parse(lex, context)?))
        }
        Some(Ok(RuleToken::OrderAuxVariables)) => {
            Ok(Box::new(OrderAuxVariablesRule::parse(lex, context)?))
        }
        Some(Ok(RuleToken::OrderDef)) => Ok(Box::new(OrderDefRule)),
        Some(Ok(RuleToken::OrderTransitivity)) => Ok(Box::new(OrderTransitivityRule)),
        Some(Ok(RuleToken::OrderReflexivity)) => Ok(Box::new(OrderReflexivityRule)),
        Some(Ok(RuleToken::OrderFreshRightVars)) => {
            Ok(Box::new(OrderFreshRightVariablesRule::parse(lex, context)?))
        }
        Some(Ok(RuleToken::OrderFreshAux1)) => {
            Ok(Box::new(OrderFreshAux1Rule::parse(lex, context)?))
        }
        Some(Ok(RuleToken::OrderFreshAux2)) => {
            Ok(Box::new(OrderFreshAux2Rule::parse(lex, context)?))
        }
        Some(Ok(RuleToken::OrderBeginProof)) => Ok(Box::new(OrderProofRule)),
        Some(Ok(RuleToken::LoadOrder)) => Ok(Box::new(LoadOrderRule::parse(lex, context)?)),
        Some(Ok(RuleToken::OrderSpecification)) => Ok(Box::new(OrderSpecificationRule)),
        Some(Ok(RuleToken::SetLevel)) => Ok(Box::new(SetLevelRule::parse(lex)?)),
        Some(Ok(RuleToken::WipeLevel)) => Ok(Box::new(Deletion::parse_level(lex)?)),
        Some(Ok(RuleToken::ObjuRule)) => Ok(Box::new(ObjectiveUpdateRule::parse(lex, context)?)),
        Some(Ok(RuleToken::EqualsObjective)) => {
            Ok(Box::new(EqualsObjectiveRule::parse(lex, context)?))
        }
        Some(Ok(RuleToken::FailProof)) => Ok(Box::new(FailRule)),
        Some(Ok(RuleToken::StrengtheningToCore)) => {
            Ok(Box::new(StrengtheningToCoreRule::parse(lex)?))
        }
        Some(Err(_)) => Err(ParserError::token_error(
            lex.span(),
            "rule identifier (e.g., 'pol', 'red', 'rup', ...) or comment",
        )),
    }
}
