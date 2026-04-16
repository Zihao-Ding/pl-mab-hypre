use std::{
    fmt::{Display, Formatter},
    rc::Rc,
};

use colored::{ColoredString, Colorize};
use itertools::Itertools;
use logos::Logos;
use veripb_formula::prelude::*;

use crate::{prelude::*, rules::ScopeId};

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum ProofgoalID {
    #[regex(r"#\d+", |lex| lex.slice()[1..].parse().ok())]
    Internal(usize),

    #[regex(r"[+-]?\d+", |lex| lex.slice().parse().ok())]
    Database(isize),
}

impl Display for ProofgoalID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProofgoalID::Internal(id) => write!(f, "#{id}"),
            ProofgoalID::Database(id) => write!(f, "{id}"),
        }
    }
}

/// [`Proofgoal`] represents a subproof proof obligation.
#[derive(Debug, Default)]
pub struct Proofgoal {
    /// The set of extra premises in the check.
    premises: Vec<Rc<DBConstraint>>,
    /// The single constraint to be checked for entailment; assumed to be False if [`None`].
    conclusion: Option<Rc<DBConstraint>>,
    /// Restrictions that proofgoal can only be used in a specific scope if a scope is loaded.
    scope_restriction: Option<ScopeId>,
    /// An optional proofgoal means that it can be proven if wanted but it is not required.
    optional: bool,
}

impl Proofgoal {
    pub fn mk_single_constraint(
        concl: Rc<DBConstraint>,
        scope_restriction: Option<ScopeId>,
        optional: bool,
    ) -> Proofgoal {
        Proofgoal {
            premises: Vec::new(),
            conclusion: Some(concl),
            scope_restriction,
            optional,
        }
    }

    pub fn mk_multi_constraint(
        premises: Vec<Rc<DBConstraint>>,
        scope_restriction: Option<ScopeId>,
    ) -> Proofgoal {
        Proofgoal {
            premises,
            conclusion: None,
            scope_restriction,
            ..Default::default()
        }
    }

    pub fn mk_mixed(
        premises: Vec<Rc<DBConstraint>>,
        concl: Rc<DBConstraint>,
        scope_restriction: Option<ScopeId>,
    ) -> Proofgoal {
        Proofgoal {
            premises,
            conclusion: Some(concl),
            scope_restriction,
            ..Default::default()
        }
    }

    pub fn into_counterexample(mut self) -> Vec<Rc<DBConstraint>> {
        if let Some(c) = self.conclusion {
            self.premises.push(Rc::new(c.negate()));
        }
        self.premises
    }

    pub fn trace(&self, idstr: &str, var_names: &VarNameManager) {
        println!(
            "proofgoal {}: {}[{}]{}{}",
            idstr.purple(),
            if self.conclusion.is_none() { "~" } else { "" },
            self.premises
                .iter()
                .map(|c| c.to_pretty_string(var_names))
                .fold("".to_string(), |out, c| out.to_string() + ", " + c.as_str()),
            if self.conclusion.is_some() {
                " |- "
            } else {
                ""
            },
            if let Some(c) = &self.conclusion {
                c.to_pretty_string(var_names).blue()
            } else {
                "".blue()
            }
        )
    }

    #[inline]
    pub fn is_in_scope(&self, scope: ScopeId) -> bool {
        if let Some(restriction) = self.scope_restriction {
            if restriction != scope {
                return false;
            }
        }
        true
    }
}

#[derive(Debug)]
pub enum ProofTechnique {
    Trivial,
    DBLookup,
    PremiseImplies,
    DBImpliesSubstituted,
    IsRUP,
    Optional,
}

impl Display for ProofTechnique {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trivial => f.write_str("trivial"),
            Self::DBLookup => f.write_str("equal to a database constraint"),
            Self::PremiseImplies => f.write_str("implied by additional premise"),
            Self::DBImpliesSubstituted => {
                f.write_str("implied by a substituted database constraint")
            }
            Self::IsRUP => f.write_str("RUP"),
            Self::Optional => f.write_str("optional and does not need to be proven"),
        }
    }
}

impl Proofgoal {
    #[inline]
    pub fn autoprove(
        &mut self,
        context: &mut Context,
        database: &mut Database,
        hint: &Option<Rc<DBConstraint>>,
        goal_id: ProofgoalID,
        elaborate_database_proofgoals: bool,
    ) -> Result<ProofTechnique, CheckingError> {
        if self.optional {
            return Ok(ProofTechnique::Optional);
        }

        match &self.conclusion {
            Some(constraint) => {
                // Check if constraint is trivial.
                if constraint.is_trivial() {
                    return Ok(ProofTechnique::Trivial);
                }

                // Check if the constraint is implied by the additional premise.
                if let Some(hint) = hint {
                    if hint.implies(constraint) {
                        return Ok(ProofTechnique::PremiseImplies);
                    }
                }

                // Check if constraint already exists in the database.
                database.update_unique_index(&mut context.propagation_engine)?;
                if let Some(found_constraint) = database.lookup(constraint) {
                    if !context.only_core || found_constraint.is_core_constraint() {
                        if elaborate_database_proofgoals {
                            if let Some(elaborator) = context.elaborator.as_mut() {
                                elaborator.write("\tproofgoal ");
                                self.elaborate_id(elaborator, goal_id);
                                let neg_proofgoal_id = elaborator.inc_id();
                                elaborator.write("\t\tpol ");
                                elaborator.write(&neg_proofgoal_id.to_string());
                                elaborator.write(" ");
                                elaborator.write(
                                    &found_constraint
                                        .get_out_id(found_constraint.get_some_id())
                                        .expect("should have output ID")
                                        .to_string(),
                                );
                                elaborator.write(" + ;\n\tqed : ");
                                let pol_id = elaborator.inc_id();
                                elaborator.write(&pol_id.to_string());
                                elaborator.writeln(";");
                            }
                        }
                        return Ok(ProofTechnique::DBLookup);
                    }
                }

                // Get the output proof buffer, as it might be required if a check succeeds.
                let mut proof_buf = None;
                if let Some(elaborator) = context.elaborator.as_mut() {
                    proof_buf = Some(&mut elaborator.proof_buf)
                }

                // Check if the constraint is RUP.
                database.update_propagation_index(&mut context.propagation_engine)?;
                let result = context.propagation_engine.reverse_unit_propagation_check(
                    &context.var_names,
                    self.premises.as_slice(),
                    Some(constraint),
                    context.only_core,
                    &mut proof_buf,
                    false,
                );
                if result?.is_conflict() {
                    if let Some(elaborator) = context.elaborator.as_mut() {
                        elaborator.write("\tproofgoal ");
                        self.elaborate_id(elaborator, goal_id);
                        elaborator.write("\t\trup >= 1 :");
                        let neg_proofgoal_id = elaborator.inc_id();
                        elaborator.replace_tilde_write_and_clear_buf(&neg_proofgoal_id.to_string());
                        elaborator.write(";\n\tqed : ");
                        let rup_id = elaborator.inc_id();
                        elaborator.write(&rup_id.to_string());
                        elaborator.writeln(";");
                    }
                    return Ok(ProofTechnique::IsRUP);
                }

                if let Some(buf) = &mut proof_buf {
                    buf.clear();
                }

                // Check if substituted constraint is implied by any constraint in the substituted database.
                if let Ok(hint) = check_substituted_implication(context, database, constraint, None)
                {
                    if let Some(elaborator) = context.elaborator.as_mut() {
                        elaborator.write("\tproofgoal ");
                        self.elaborate_id(elaborator, goal_id);
                        let neg_proofgoal_id = elaborator.inc_id();

                        // Get substitution as constraint.
                        let trail = context.propagation_engine.get_trail();
                        context
                            .propagation_engine
                            .get_propagated_assignment_hints(&mut elaborator.proof_buf);
                        elaborator.write("\t\trup ");
                        elaborator.write(
                            &trail
                                .trail
                                .iter()
                                .map(|prop| {
                                    format!("1 {}", prop.lit.to_pretty_string(&context.var_names))
                                })
                                .join(" "),
                        );
                        elaborator.write(&format!(" >= {} : ", trail.len()));
                        elaborator.write_and_clear_buf();
                        elaborator.writeln(" ~ ;");
                        let units_constraint_id = elaborator.inc_id();

                        // Apply substitution to premise.
                        let premise = database.get_entry_usize(hint as usize)?;
                        elaborator.writeln(&format!(
                            "\t\tpol {} {} {} * +;",
                            premise.get_out_id(premise.get_some_id()).unwrap(),
                            units_constraint_id,
                            premise.get_max_coeff()
                        ));
                        let sub_premise_id = elaborator.inc_id();

                        // Perform the syntactic implication.
                        let sub_constraint = constraint.substitute(&trail.assignment);
                        elaborator.write("\t\tia ");
                        elaborator.write(&sub_constraint.to_pretty_string(&context.var_names));
                        elaborator.write(" : ");
                        elaborator.write(&sub_premise_id.to_string());
                        let sub_implication_id = elaborator.inc_id();

                        // Derive contradiction
                        elaborator.write(" ;\n\t\tpol ");
                        elaborator.write(&format!(
                            "{} {} {} * + ",
                            neg_proofgoal_id,
                            units_constraint_id,
                            constraint.get_max_coeff(),
                        ));
                        elaborator.write(&sub_implication_id.to_string());
                        elaborator.writeln(" + ;");
                        elaborator.write("\tqed : ");
                        let pol_id = elaborator.inc_id();
                        elaborator.write(&pol_id.to_string());
                        elaborator.writeln(";");
                    }
                    return Ok(ProofTechnique::DBImpliesSubstituted);
                }
            }
            None => {
                // If we do elaboration, add output constraint IDs to goal constraints and get the output proof buffer, as it might be required if a check succeeds.
                let mut proof_buf = None;
                if let Some(elaborator) = context.elaborator.as_mut() {
                    for constraint in self.premises.iter_mut() {
                        constraint.set_out_id(0, elaborator.inc_id());
                    }
                    proof_buf = Some(&mut elaborator.proof_buf)
                }

                // Check if proofgoal constraints are RUP.
                database.update_propagation_index(&mut context.propagation_engine)?;
                if context
                    .propagation_engine
                    .reverse_unit_propagation_check(
                        &context.var_names,
                        self.premises.as_slice(),
                        None,
                        context.only_core,
                        &mut proof_buf,
                        context.args.trace_failed,
                    )?
                    .is_conflict()
                {
                    if let Some(elaborator) = context.elaborator.as_mut() {
                        elaborator.write("\tproofgoal ");
                        self.elaborate_id(elaborator, goal_id);
                        elaborator.write("\t\trup >= 1 :");
                        elaborator.write_and_clear_buf();
                        elaborator.write(";\n\tqed : ");
                        let rup_id = elaborator.inc_id();
                        elaborator.write(&rup_id.to_string());
                        elaborator.writeln(";");
                    }
                    return Ok(ProofTechnique::IsRUP);
                }
            }
        }
        Err(CheckingError::AutoprovingFailed(goal_id))
    }

    #[inline]
    pub fn trace_autoproven(&self, colored_id: ColoredString, technique: ProofTechnique) {
        println!("    proofgoal {colored_id} is {technique}");
    }

    #[inline]
    pub fn conclusion(&self) -> Option<&Rc<DBConstraint>> {
        self.conclusion.as_ref()
    }

    #[inline]
    pub fn unwrap_conclusion(&self) -> &Rc<DBConstraint> {
        self.conclusion()
            .expect("Expected a proofgoal with a specified conclusion!")
    }

    #[inline]
    fn elaborate_id(&self, elaborator: &mut Elaborator, goal_id: ProofgoalID) {
        match goal_id {
            ProofgoalID::Internal(id) => {
                elaborator.write("#");
                elaborator.writeln(&id.to_string());
            }
            ProofgoalID::Database(id) => elaborator.writeln(
                &self
                    .unwrap_conclusion()
                    .get_out_id(id as usize)
                    .expect("database proofgoal should have same IDs as database constraint")
                    .to_string(),
            ),
        }
    }
}
