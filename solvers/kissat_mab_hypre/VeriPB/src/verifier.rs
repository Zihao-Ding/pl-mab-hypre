use std::io::Error;

use crate::{error::VeriPBError, prelude::Elaborator, Rule};
use colored::Colorize;
use logos::Logos;
use veripb_formula::prelude::*;
use veripb_parser::parser::get_lines;

use crate::{
    context::{Context, CORE},
    database::Database,
    error::CheckingError,
    rules::{line_to_rule, RuleToken},
};

/// Main data structure of the VeriPB verifier.
///
/// This data structure maintains the invariants of the proof.
///
/// To initalize the data structure, create a new `Verified` using [`Verifier::new()`] and then call [`Verifier::initialize()`].
///
/// A rule can be checked using [`Verifier::execute_rule()`]
pub struct Verifier {
    pub context: Context,
    pub database: Database,
    returned_constraint_id: Option<isize>,
}

impl Verifier {
    /// Create a new `Verifier` from an existing [`Context`] and a [`Formula`].
    ///
    /// The `formula` is used to initialize the database. The elaborator is initialized based the elaborated proof file path given in the `args` of `context`
    pub fn new(mut context: Context, formula: Formula) -> Result<Self, Error> {
        // Initialize elaborator if needed.
        if let Some(output_file) = context.args.elaborate.clone() {
            let mut elaborator = Elaborator::new(output_file)?;
            elaborator.load_formula(formula.len());
            context.elaborator = Some(elaborator);
        }

        // Initialize database.
        let mut database = Database::from_formula(formula.constraints);
        context.objective = formula.objective.clone();

        // Set up output constraint IDs for original constraints.
        if let Some(elaborator) = context.elaborator.as_mut() {
            for (orig_id, constraint) in database.entries.iter_mut().flatten().enumerate() {
                constraint.set_out_id(orig_id + 1, elaborator.inc_id());
            }
        }

        // Keep original constraints and objective.
        context.original_constraints = database.unique_constraints.iter().cloned().collect();
        context.original_objective = formula.objective;

        let verifier = Verifier {
            context,
            database,
            returned_constraint_id: None,
        };

        if verifier.context.args.trace {
            verifier.print_formula();
        }

        Ok(verifier)
    }

    /// Initialize the propagation engine in the [`Verifier`].
    pub fn initialize(&mut self) -> anyhow::Result<()> {
        // Initialize the propagation engine.
        for unique_constraint in self.database.unique_constraints.iter() {
            self.context
                .propagation_engine
                .attach(CORE, unique_constraint)?;
        }
        Ok(())
    }

    /// Use the version 2.x parser for VeriPB proof files and check the deriviation.
    pub fn verify_file_version_2(&mut self) -> anyhow::Result<()> {
        // Iterate over the lines of the proof.
        let proof_lines = get_lines(&self.context.args.derivation)?;
        for (line_number, line) in proof_lines.map_while(Result::ok).enumerate() {
            let lex = RuleToken::lexer(&line);
            if self.context.args.trace {
                println!(
                    "{}",
                    format!("line {:>4}: {}", line_number + 1, lex.source()).bright_black(),
                );
            }

            // Parse the new rule.
            let rule = anyhow::Context::with_context(line_to_rule(lex, &mut self.context), || {
                format!(
                    "Parsing error at {}:{}",
                    self.context.args.derivation.to_string_lossy(),
                    line_number + 1
                )
            })?;

            self.execute_rule(line_number, rule)?;
        }

        if !self.context.has_end_proof {
            return Err(CheckingError::EndProofMissing.into());
        }

        Ok(())
    }

    /// Get the last returned constraint ID.
    ///
    /// This ID is usually the ID of the constraint that was just derived. For the `e` (equals) rule this is the ID of the constraint to which the given constraint is equal.
    pub fn get_returned_constraint_id(&mut self) -> Option<isize> {
        let result = self.returned_constraint_id;
        self.returned_constraint_id = None;
        result
    }

    /// Execute a rule.
    ///
    /// This performs the following checking in order:
    /// 1. Preliminary checks that this rule can be used.
    /// 2. Check that the rule can be applied and calculated the derived constraint.
    /// 3. Add the derived constraints to the database.
    /// 4. Elaborate the rule.
    /// 5. Delete constraints deleted by this rule.
    /// 6. Change the database.
    pub fn execute_rule(
        &mut self,
        line: usize,
        mut rule: Box<dyn Rule>,
    ) -> Result<(), VeriPBError> {
        // Check if we are in a subproof and if the rule can be used within a subproof.
        let last_subcontext = self.context.subcontexts.last();
        if last_subcontext.is_some()
            && last_subcontext.unwrap().is_subproof()
            && !rule.is_subproof_friendly()
        {
            return Err(CheckingError::RuleNotSubproofFriendly(
                std::any::type_name_of_val(&rule).to_string(),
            )
            .add_context(self.context.args.derivation.clone(), line));
        }

        // Do the checks for the rule.
        let new_constraints = rule
            .compute(&mut self.context, &mut self.database)
            .map_err(|e| e.add_context(self.context.args.derivation.clone(), line))?;
        let num_new_constraints = new_constraints.len();

        // Add the new constraints.
        for constraint in new_constraints {
            if self.context.args.trace {
                println!(
                    "  ConstraintId {}: {}",
                    self.database.len().to_string().bright_green(),
                    &constraint
                        .constraint
                        .to_pretty_string(&self.context.var_names)
                        .blue()
                )
            }
            // Add constraint ID to current level.
            if let Some(level) = self.context.current_level {
                unsafe {
                    self.context
                        .level_ids
                        .get_unchecked_mut(level)
                        .push(self.database.len())
                };
            }
            // Add constraint to database.
            self.database.add_constraint(
                constraint,
                self.context.only_core || rule.add_constraints_to_core(&self.context),
            );
        }

        // Get ID of returned constraint.
        self.returned_constraint_id = rule
            .get_returned_id()
            .or_else(|| Some(std::cmp::max(self.database.len(), 1) as isize - 1));

        // Elaborate the rule.
        if let Some(elaborator) = self.context.elaborator.as_mut() {
            // Add output constraint ID for elaborated proof.
            let mut start_id_new_constraints = self.database.len() - num_new_constraints;
            for constraint in self.database.entries[start_id_new_constraints..]
                .iter()
                .flatten()
            {
                constraint.set_out_id(start_id_new_constraints, elaborator.inc_id());
                start_id_new_constraints += 1;
            }
            rule.elaborate(&mut self.context, &self.database)?;
        }

        // Delete constraints also implicit through ending subproofs.
        if let Some(deletion_sequence) = rule
            .get_deleted(&self.database)
            .map_err(|e| e.add_context(self.context.args.derivation.clone(), line))?
        {
            if self.context.args.trace {
                if let Some(str) = deletion_sequence.trace_deletions() {
                    println!("  Deleting IDs: {str}");
                }
            }

            deletion_sequence
                .delete_constraints(&mut self.database, &mut self.context)
                .map_err(|e| e.add_context(self.context.args.derivation.clone(), line))?
        }

        // Change the currently considered database.
        if rule.swaps_database() {
            let (database, prop_engine) = rule.get_new_database();
            let old_database = std::mem::replace(&mut self.database, database);
            let old_prop_engine =
                std::mem::replace(&mut self.context.propagation_engine, prop_engine);
            rule.handle_old_database(&mut self.context, old_database, old_prop_engine);
        }

        Ok(())
    }

    /// Print the formula to the trace.
    fn print_formula(&self) {
        if let Some(objective) = &self.context.objective {
            println!(
                "  {}: {}",
                "Objective".bright_green(),
                ("min ".to_owned() + objective.to_pretty_string(&self.context.var_names).as_str())
                    .green()
            )
        }
        for (idx, constraint) in self.database.entries.iter().enumerate() {
            if let Some(constraint) = constraint {
                println!(
                    "  ConstraintId {}: {}",
                    idx.to_string().bright_green(),
                    constraint
                        .constraint
                        .to_pretty_string(&self.context.var_names)
                        .blue()
                )
            }
        }
    }
}
