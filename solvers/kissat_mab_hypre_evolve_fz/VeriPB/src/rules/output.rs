use std::{ffi::OsStr, rc::Rc};

use logos::{Lexer, Logos};
use veripb_formula::prelude::*;
use veripb_parser::{
    error::ParserError, opb_parser::parse_opb_from_file_given_var_manager,
    wcnf_parser::parse_wcnf_from_file_given_var_manager,
};

use crate::prelude::*;

const DERIVABLE: &str = "DERIVABLE";
const EQUISAT: &str = "EQUISATISFIABLE";
const EQUIOPT: &str = "EQUIOPTIMAL";
const EQUIENUM: &str = "EQUIENUMERABLE";
const NONE: &str = "NONE";

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum OutputGuarantee {
    #[token("DERIVABLE")]
    Derivable,

    #[token("EQUISATISFIABLE")]
    Equisatisfiable,

    #[token("EQUIOPTIMAL")]
    Equioptimal,

    #[token("EQUIENUMERABLE")]
    Equienumerable,

    #[token("NONE")]
    None,
}

const IMPLICIT: &str = "IMPLICIT;";
const FILE: &str = "FILE;";
const PERMUTATION: &str = "PERMUTATION;";

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
pub enum OutputType {
    #[token("IMPLICIT")]
    Implicit,

    #[token("FILE")]
    File,

    #[token("PERMUTATION")]
    Permutation,

    None,
}

#[inline]
fn parse_output_file(context: &mut Context) -> Result<Formula, CheckingError> {
    if let Some(output_formula_path) = &context.args.output_formula {
        if context.args.cnf
            || (!context.args.opb
                && !context.args.wcnf
                && output_formula_path.extension() == Some(OsStr::new("cnf")))
        {
            unimplemented!("CNF output formulas are not supported")
        } else if context.args.wcnf
            || (!context.args.opb && output_formula_path.extension() == Some(OsStr::new("wcnf")))
        {
            Ok(parse_wcnf_from_file_given_var_manager(
                output_formula_path,
                &mut context.var_names,
            )?)
        } else {
            Ok(
                parse_opb_from_file_given_var_manager(output_formula_path, &mut context.var_names)?
                    .0,
            )
        }
    } else {
        Err(CheckingError::NoOutputFormulaGiven)
    }
}

#[inline]
fn check_file(context: &mut Context, database: &mut Database) -> Result<(), CheckingError> {
    // Parse the output formula.
    let output_formula = parse_output_file(context)?;

    // Check that the objectives are equivalent.
    match (&context.objective, output_formula.objective) {
        (None, None) => {}
        (Some(current_objective), Some(output_objective)) => {
            if *current_objective != output_objective {
                return Err(CheckingError::OutputObjectiveMismatch);
            }
        }
        (None, Some(_)) => return Err(CheckingError::OutputObjectiveButNoProofObjective),
        (Some(_), None) => return Err(CheckingError::OutputNoObjectiveButProofObjective),
    }

    // Check if every constraint in output formula is in core constraints.
    database.update_unique_index(&mut context.propagation_engine)?;
    for output_constraint in output_formula.constraints {
        let output_constraint = Rc::new(output_constraint.into());
        if let Some(database_constraint) = database.lookup(&output_constraint) {
            if database_constraint.is_core_constraint() {
                // Mark constraints for check that every core constraint is in output formula.
                database_constraint.header.borrow_mut().is_in_output_formula = true;
            } else {
                return Err(CheckingError::OutputConstraintNotInCore(
                    database_constraint.get_some_id(),
                ));
            }
        } else {
            return Err(CheckingError::OutputConstraintNotInDatabase(
                output_constraint.into(),
            ));
        }
    }

    // Check if every core constraint was marked by a output formula constraint.
    for constraint in database.unique_constraints.iter() {
        if constraint.is_core_and_not_in_output_constraint() {
            return Err(CheckingError::OutputCoreConstraintNotInOutput(
                constraint.get_some_id(),
            ));
        }
    }

    Ok(())
}

#[derive(Debug)]
pub struct OutputRule {
    guarantee: OutputGuarantee,
    output_type: OutputType,
}

impl OutputRule {
    pub fn new(guarantee: OutputGuarantee, output_type: OutputType) -> Self {
        Self {
            guarantee,
            output_type,
        }
    }

    pub fn parse(lex: Lexer<RuleToken>) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let guarantee = match lex.next() {
            Some(Ok(guarantee)) => guarantee,
            _ => {
                return Err(ParserError::token_error(
                    lex.span(),
                    &format!("output guarantee (e.g. '{NONE}', '{DERIVABLE}', ...)"),
                ))
            }
        };

        let mut lex = lex.morph();
        let output_type = match lex.next() {
            Some(Ok(output_type)) => output_type,
            None => {
                if guarantee != OutputGuarantee::None {
                    return Err(ParserError::token_error(
                        lex.span(),
                        &format!("output type (e.g. '{NONE}', '{FILE}', ...)"),
                    ));
                }
                OutputType::None
            }
            _ => {
                return Err(ParserError::token_error(
                    lex.span(),
                    &format!("output type (e.g. '{NONE}', '{FILE}', ...)"),
                ))
            }
        };

        Ok(OutputRule {
            guarantee,
            output_type,
        })
    }
}

impl Rule for OutputRule {
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if context.has_output {
            return Err(CheckingError::DoubleOutput);
        }
        if context.has_conclusion || context.has_end_proof {
            return Err(CheckingError::WrongFooterOrder("output"));
        }
        if !context.subcontexts.is_empty() {
            return Err(CheckingError::OutputWhileOpenSubcontext);
        }

        // Check guarantee conditions.
        match self.guarantee {
            OutputGuarantee::Equioptimal
            | OutputGuarantee::Equisatisfiable
            | OutputGuarantee::Equienumerable => {
                if !context.args.checked_deletion {
                    return Err(CheckingError::ExpectedCheckDeletion);
                }
            }
            _ => {}
        }

        // Check that right guarantee was used.
        match self.guarantee {
            OutputGuarantee::Equioptimal => {
                if context.objective.is_none() {
                    return Err(CheckingError::OutputEquioptimalWithoutObjective);
                }
            }
            OutputGuarantee::Equisatisfiable => {
                if context.objective.is_some() {
                    return Err(CheckingError::OutputEquisatisfiableWithObjective);
                }
            }
            OutputGuarantee::Equienumerable => {
                unimplemented!()
            }
            _ => {}
        }

        // Check that the output is equivalent to the current problem.
        match self.output_type {
            OutputType::None => {}
            OutputType::Implicit => {}
            OutputType::File => check_file(context, database)?,
            _ => {
                unimplemented!()
            }
        }

        if context.args.print_verification_result {
            match self.guarantee {
                OutputGuarantee::Derivable => println!("s VERIFIED OUTPUT {DERIVABLE}"),
                OutputGuarantee::Equioptimal => {
                    println!("s VERIFIED OUTPUT {EQUIOPT} FOR obj < ???")
                }
                OutputGuarantee::Equisatisfiable => println!("s VERIFIED OUTPUT {EQUISAT}"),
                OutputGuarantee::Equienumerable => println!("s VERIFIED OUTPUT {EQUIENUM}"),
                _ => {}
            }
        }

        context.has_output = true;
        Ok(vec![])
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.write("output ");
        match self.guarantee {
            OutputGuarantee::None => elaborator.write(NONE),
            OutputGuarantee::Derivable => elaborator.write(DERIVABLE),
            OutputGuarantee::Equisatisfiable => elaborator.write(EQUISAT),
            OutputGuarantee::Equioptimal => elaborator.write(EQUIOPT),
            OutputGuarantee::Equienumerable => elaborator.write(EQUIENUM),
        }
        elaborator.write(" ");
        match self.output_type {
            OutputType::Implicit => elaborator.writeln(IMPLICIT),
            OutputType::File => elaborator.writeln(FILE),
            OutputType::Permutation => elaborator.writeln(PERMUTATION),
            OutputType::None => elaborator.writeln(";"),
        }
        Ok(())
    }
}
