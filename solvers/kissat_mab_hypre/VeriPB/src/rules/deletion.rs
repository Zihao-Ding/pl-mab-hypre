use std::rc::Rc;

use colored::Colorize;
use logos::Lexer;
use veripb_formula::prelude::*;
use veripb_parser::{
    error::ParserError, opb_parser::parse_single_constraint,
    substitution_parser::parse_substitution,
};

use super::Rule;

use crate::{
    context::REQUIRED_RUP_STREAK,
    deletion_sequence::{DeletionSequence, DeletionSequenceEnum},
    prelude::*,
    rules::ScopeId,
};

#[derive(Debug, PartialEq)]
pub enum DeletionOption {
    Id(Vec<isize>),
    Range((isize, isize)),
    Spec(Rc<DBConstraint>),
    Wipe(usize),
}

#[derive(Debug)]
pub enum DeletionOrigin {
    Unknown,
    Core,
    Derived,
}

#[derive(Debug)]
pub struct Deletion {
    identifier: DeletionOption,
    witness: Substitution,
    has_subproof: bool,
    origin: DeletionOrigin,
    core_ids: Vec<usize>,
    derived_ids: Vec<usize>,
}

impl Deletion {
    pub fn new(
        identifier: DeletionOption,
        witness: Substitution,
        has_subproof: bool,
        origin: DeletionOrigin,
    ) -> Self {
        Self {
            identifier,
            witness,
            has_subproof,
            origin,
            core_ids: Vec::new(),
            derived_ids: Vec::new(),
        }
    }

    pub fn parse_level(lex: Lexer<RuleToken>) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let level = IntegerToken::parse(&mut lex)?;
        if level.is_negative() {
            return Err(ParserError::token_error(lex.span(), "non-negative integer"));
        }
        Ok(Deletion {
            identifier: DeletionOption::Wipe(level as usize),
            witness: Substitution::default(),
            has_subproof: false,
            origin: DeletionOrigin::Unknown,
            core_ids: Vec::new(),
            derived_ids: Vec::new(),
        })
    }

    pub fn parse_ids(
        lex: Lexer<RuleToken>,
        context: &mut Context,
        origin: DeletionOrigin,
    ) -> Result<Self, ParserError> {
        let mut constraint_ids = Vec::new();
        let mut lex = lex.morph();
        while let Some(constraint_id) = IntegerOrSemicolonToken::parse(&mut lex)? {
            constraint_ids.push(constraint_id);
        }
        // Remove duplicate constraint IDs.
        constraint_ids.sort_unstable();
        constraint_ids.dedup();
        let mut lex = lex.morph();
        let witness = parse_substitution(&mut lex, &mut context.var_names)?;
        let has_subproof = SubproofBeginToken::parsed_begin(&mut lex.morph())?;

        Ok(Deletion {
            identifier: DeletionOption::Id(constraint_ids),
            witness,
            has_subproof,
            origin,
            core_ids: Vec::new(),
            derived_ids: Vec::new(),
        })
    }

    pub fn parse(lex: Lexer<RuleToken>, context: &mut Context) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let witness;
        let has_subproof;
        let identifier = match IdentifierOption::parse(&mut lex)? {
            IdentifierOption::Id => {
                let mut constraint_ids = Vec::new();
                let mut lex = lex.morph();
                while let Some(constraint_id) = IntegerOrSemicolonToken::parse(&mut lex)? {
                    constraint_ids.push(constraint_id);
                }
                // Remove duplicate constraint IDs.
                constraint_ids.sort_unstable();
                constraint_ids.dedup();
                // This is should be changed in the future and moved outside the match statement, but this is currently not possible.
                let mut lex = lex.morph();
                witness = parse_substitution(&mut lex, &mut context.var_names)?;
                has_subproof = SubproofBeginToken::parsed_begin(&mut lex.morph())?;
                DeletionOption::Id(constraint_ids)
            }
            IdentifierOption::Range => {
                let mut lex = lex.morph();
                let start_id = IntegerToken::parse(&mut lex)?;
                let end_id = IntegerToken::parse(&mut lex)?;
                // This is should be changed in the future and moved outside the match statement, but this is currently not possible.
                let mut lex = lex.morph();
                witness = parse_substitution(&mut lex, &mut context.var_names)?;
                has_subproof = SubproofBeginToken::parsed_begin(&mut lex.morph())?;
                DeletionOption::Range((start_id, end_id))
            }
            IdentifierOption::Specification => {
                let mut lex = lex.morph();
                let (constraint, leq_constraint) =
                    parse_single_constraint(&mut lex, &mut context.var_names)?;
                if leq_constraint.is_some() {
                    return Err(ParserError::token_error(
                        0..lex.span().end,
                        "inequality constraint",
                    ));
                }
                // This is should be changed in the future and moved outside the match statement, but this is currently not possible.
                let mut lex = lex.morph();
                witness = parse_substitution(&mut lex, &mut context.var_names)?;
                has_subproof = SubproofBeginToken::parsed_begin(&mut lex.morph())?;
                DeletionOption::Spec(Rc::new(DBConstraint::from(constraint)))
            }
        };

        Ok(Deletion {
            identifier,
            witness,
            has_subproof,
            origin: DeletionOrigin::Unknown,
            core_ids: Vec::new(),
            derived_ids: Vec::new(),
        })
    }

    /// Get the actual deletions due to this deletion command.
    ///
    /// This is required, as we allow deletion by specification and deletion by ID in the same proof. Hence, we follow the following convention for deletions.
    ///
    /// - If a constraint is deleted by specification and there are more valid constraint IDs for this constraints than the number of times this constraint has been deleted by specification, then all constraint IDs stay valid. Otherwise, all constraint IDs become invalid and the constraint is deleted.
    /// - If a constraint is deleted by ID, then this ID is no longer valid for this constraint. If this causes that the number of valid IDs is at least the number deletions by specification, then all constraint IDs become invalid and the constraint is deleted.
    ///
    /// This function returns `true`, if at least one constraint ID that is actually deleted is in the core set.
    #[inline]
    fn get_actual_deletions(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<(), CheckingError> {
        match &self.identifier {
            DeletionOption::Id(constraint_ids) => {
                for &id in constraint_ids {
                    let index = database.normalize_id(id) as usize;
                    let constraint = database.get_entry_usize(index)?;
                    let (core_ids, derived_ids) = constraint.get_del_by_id(index);
                    for id in core_ids {
                        self.core_ids.push(id);
                    }
                    for id in derived_ids {
                        self.derived_ids.push(id);
                    }
                }
            }
            DeletionOption::Range((start, end)) => {
                let start_index = database.normalize_id(*start) as usize;
                let end_index = database.normalize_id(*end) as usize;
                for index_result in database.get_undeleted(start_index..end_index) {
                    let index = index_result?;
                    let constraint = database.get_entry_usize(index)?;
                    let (core_ids, derived_ids) = constraint.get_del_by_id(index);
                    for id in core_ids {
                        self.core_ids.push(id);
                    }
                    for id in derived_ids {
                        self.derived_ids.push(id);
                    }
                }
            }
            DeletionOption::Spec(constraint) => {
                database.update_unique_index(&mut context.propagation_engine)?;
                if let Some(db_constraint) = database.lookup(constraint) {
                    let (core_ids, derived_ids) = db_constraint.get_del_by_spec();
                    for id in core_ids {
                        self.core_ids.push(id);
                    }
                    for id in derived_ids {
                        self.derived_ids.push(id);
                    }
                } else {
                    return Err(CheckingError::not_found(constraint, false));
                }
            }
            DeletionOption::Wipe(level) => {
                let number_levels = context.level_ids.len();
                for ids in &mut context.level_ids[*level..number_levels] {
                    for index in ids.drain(..) {
                        if let Some(constraint) =
                            database.get_entry_optionally_deleted_usize(index)?
                        {
                            let (core_ids, derived_ids) = constraint.get_del_by_id(index);
                            for id in core_ids {
                                self.core_ids.push(id);
                            }
                            for id in derived_ids {
                                self.derived_ids.push(id);
                            }
                        }
                    }
                }
            }
        }
        // Remove duplicates from the actually deleted IDs.
        self.core_ids.sort_unstable();
        self.core_ids.dedup();
        self.derived_ids.sort_unstable();
        self.derived_ids.dedup();

        Ok(())
    }

    /// Check if deletion from core and deletion from derived set only deletes constraints from their respective set.
    #[inline]
    fn check_deletion_origin(&self) -> Result<(), CheckingError> {
        match self.origin {
            DeletionOrigin::Core => {
                if !self.derived_ids.is_empty() {
                    return Err(CheckingError::DeletionFromCoreDeletesDerived(
                        self.derived_ids[0],
                    ));
                }
            }
            DeletionOrigin::Derived => {
                if !self.core_ids.is_empty() {
                    return Err(CheckingError::DeletionFromDerivedDeletesCore(
                        self.core_ids[0],
                    ));
                }
            }
            DeletionOrigin::Unknown => {}
        }
        Ok(())
    }

    /// Unchecked deletion can be used if all of the following conditions are met:
    /// (0. No explicit subproof is specified, but this might only be a temporary condition.)
    /// 1. The order is empty or move all constraints to core.
    #[inline]
    fn check_unchecked_deletion(
        &self,
        database: &Database,
        context: &mut Context,
    ) -> Result<(), CheckingError> {
        if self.has_subproof {
            return Err(CheckingError::UncheckedWithSubproof);
        }

        if context.active_order.is_some() || context.is_strengthening_to_core {
            database.move_to_core_all(&mut context.propagation_engine)?;
        }

        Ok(())
    }

    /// Checked deletion check. We need to construct the proof goals and use heuristics similar to redundance-based strengthening.
    #[inline]
    fn check_checked_deletion(
        &mut self,
        constraint_id: usize,
        database: &mut Database,
        context: &mut Context,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        let deleted_constraint = database.get_entry_usize(constraint_id)?.clone();
        let out_id = deleted_constraint.get_out_id(constraint_id);

        // Remove constraint from database and from proapgator.
        database.delete_constraint(context, constraint_id)?;

        // Maybe the deletion is actually just implied by RUP. We only do this check if we had enough success autoproving the subproof at top-level without using the witness.
        if !self.has_subproof
            && (context.rup_streak >= REQUIRED_RUP_STREAK || self.witness.is_empty())
        {
            // Check if constraint is in DB.
            database.update_unique_index(&mut context.propagation_engine)?;
            if let Some(constraint) = database.lookup(&deleted_constraint) {
                if constraint.is_core_constraint() {
                    if context.args.trace {
                        println!("* constraint is in DB already")
                    }
                    if let Some(elaborator) = context.elaborator.as_mut() {
                        elaborator.writeln(&format!("delc {} : : subproof", out_id.unwrap()));
                        elaborator.write("\tpol ");
                        elaborator.write_inc_id();
                        elaborator.write(" ");
                        elaborator.write(
                            &constraint
                                .get_out_id(constraint.get_some_id())
                                .unwrap()
                                .to_string(),
                        );
                        elaborator.writeln(" + ;");
                        elaborator.write("qed : ");
                        elaborator.write_inc_id();
                        elaborator.writeln(" ;");
                    }
                    return Ok(vec![]);
                }
            }
            // Check if constraint is RUP.
            let mut proof_buf = None;
            if let Some(elaborator) = context.elaborator.as_mut() {
                proof_buf = Some(&mut elaborator.proof_buf)
            }
            database.update_propagation_index(&mut context.propagation_engine)?;
            if context
                .propagation_engine
                .reverse_unit_propagation_check(
                    &context.var_names,
                    &[],
                    Some(&deleted_constraint),
                    true,
                    &mut proof_buf,
                    false,
                )?
                .is_conflict()
            {
                if context.args.trace {
                    println!("* constraint is RUP *")
                }
                if let Some(elaborator) = context.elaborator.as_mut() {
                    elaborator.writeln(&format!("delc {} : : subproof", out_id.unwrap()));
                    elaborator.write("\trup >= 1 :");
                    let next_id = elaborator.inc_id();
                    elaborator.proof_buf = elaborator.proof_buf.replace("~", &next_id.to_string());
                    elaborator.write_and_clear_buf();
                    elaborator.writeln(";");
                    elaborator.write("qed : ");
                    elaborator.write_inc_id();
                    elaborator.writeln(" ;");
                }
                return Ok(vec![]);
            }
            context.rup_streak = std::num::Saturating(0);
        }

        let mut subproof_context = SubproofContext::new(database.len());

        // We need to elaborate the red rule line here, as autoproving might generate extra lines.
        if let Some(elaborator) = &mut context.elaborator {
            elaborator.writeln(&format!(
                "delc {} : {} : subproof",
                out_id.unwrap(),
                self.witness.to_pretty_string(&context.var_names)
            ));
        }

        if context.is_strengthening_to_core {
            subproof_context.proof_by_contradiction_subproof = true;
        }
        if !context.is_strengthening_to_core || context.major_version == Some(2) {
            // Generate the proofgoals, starting with the constraint to be introduced.
            subproof_context.add_single_proofgoal(
                Rc::new(deleted_constraint.substitute(&self.witness)),
                Some(ScopeId::LessEqual),
            );
            if context.args.trace {
                println!("  ** proofgoal from satisfying deleted constraint **");
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

            // Get core database proofgoals.
            database.update_unique_index(&mut context.propagation_engine)?;
            for constraint in
                database.get_proofgoals(&self.witness, false, context.elaborator.is_some(), true)
            {
                subproof_context.add_database_proofgoal(
                    constraint,
                    false,
                    Some(ScopeId::LessEqual),
                );
            }
            if context.args.trace {
                subproof_context.trace_database_proofgoals(&context.var_names);
            }
        }

        let negated = Rc::new(deleted_constraint.negate());

        if self.has_subproof {
            subproof_context.additional_hint = Some(database.len());
            context.inside_strengthening_subproof = true;
            subproof_context.witness = std::mem::take(&mut self.witness);
            context
                .subcontexts
                .push(Subcontext::Subproof(subproof_context));
            Ok(vec![negated])
        } else {
            if let Some(elaborator) = context.elaborator.as_mut() {
                negated.set_out_id(0, elaborator.inc_id());
            }

            subproof_context
                .finalize(context, database, &[negated])
                .map_err(|e| match e {
                    CheckingError::AutoprovingFailed(goal_id) => {
                        CheckingError::CheckedDeletionAutoprovingFailed(goal_id, out_id)
                    }
                    e => e,
                })?;
            Ok(vec![])
        }
    }
}

impl Rule for Deletion {
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        // Get what constraints and IDs are actually deleted by this rule.
        self.get_actual_deletions(context, database)?;
        // Get which deleted IDs are from the core and which are from the derived set.
        self.check_deletion_origin()?;

        // Check preconditions:
        // If there is a subproof, then there should be exactly one core constraint.
        if self.has_subproof && self.core_ids.len() != 1 {
            return Err(CheckingError::DeletionSubproofNotOneCoreConstraint);
        }
        // If there are more than one core constraint, then the witness must be empty.
        if self.core_ids.len() > 1 && !self.witness.is_empty() {
            return Err(CheckingError::DeletionMultipleCoreWithWitness);
        }
        // If strengthening to core is enabled, then checked deletion cannot use a witness.
        if context.is_strengthening_to_core && !self.core_ids.is_empty() && !self.witness.is_empty()
        {
            return Err(CheckingError::DeletionWithWitnessWhileStrengtheningToCore);
        }

        if context.args.checked_deletion {
            context.only_core = true;
            while let Some(id) = self.core_ids.pop() {
                if context.args.trace {
                    println!(
                        "  Checked deletion of ID: {}",
                        id.to_string().bright_green()
                    );
                }
                if self.has_subproof {
                    return self.check_checked_deletion(id, database, context);
                } else {
                    if let Some(elaborator) = &mut context.elaborator {
                        elaborator.enable_buffered_proof();
                    }
                    match self.check_checked_deletion(id, database, context) {
                        Ok(_) => {
                            if let Some(elaborator) = &mut context.elaborator {
                                elaborator.write_buffered_proof();
                            }
                        }
                        Err(CheckingError::CheckedDeletionAutoprovingFailed(goal_id, out_id)) => {
                            if context.args.force_checked_deletion {
                                return Err(CheckingError::ForceCheckedDeletionFailed(goal_id));
                            }
                            eprintln!("Warning: Switching from checked to unchecked deletion.");
                            if let Some(elaborator) = &mut context.elaborator {
                                elaborator.forget_buffered_proof();
                                elaborator.writeln(&format!("delc {};", out_id.unwrap()));
                            }
                            context.args.checked_deletion = false;
                            self.check_unchecked_deletion(database, context)?;
                            break;
                        }
                        e => return e,
                    }
                }
            }
            context.only_core = false;
        } else if !self.core_ids.is_empty() {
            self.check_unchecked_deletion(database, context)?;
        }

        Ok(vec![])
    }

    fn get_deleted(
        &self,
        _database: &Database,
    ) -> Result<Option<DeletionSequenceEnum<'_>>, CheckingError> {
        Ok(Some(DeletionSequenceEnum::Deletion(self)))
    }

    fn elaborate(
        &self,
        context: &mut Context,
        database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();

        if !self.derived_ids.is_empty() {
            elaborator.write("deld");
            for &index in self.derived_ids.iter() {
                let constraint = database
                    .get_entry_usize(index)
                    .expect("constraint already accessed");
                elaborator.write(" ");
                elaborator.write(
                    &constraint
                        .get_out_id(index)
                        .expect("should have out ID")
                        .to_string(),
                );
            }
            elaborator.writeln(";");
        }

        if !context.args.checked_deletion && !self.core_ids.is_empty() {
            elaborator.write("delc");
            for &index in self.core_ids.iter() {
                let constraint = database
                    .get_entry_usize(index)
                    .expect("constraint already accessed");
                elaborator.write(" ");
                elaborator.write(
                    &constraint
                        .get_out_id(index)
                        .expect("should have out ID")
                        .to_string(),
                );
            }
            elaborator.writeln(";");
        }

        Ok(())
    }
}

impl DeletionSequence for Deletion {
    #[inline]
    fn get_deleted_ids(&self) -> impl Iterator<Item = &usize> {
        self.core_ids.iter().chain(self.derived_ids.iter())
    }
}
