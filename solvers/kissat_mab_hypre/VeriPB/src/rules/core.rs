use std::rc::Rc;

use logos::Lexer;
use veripb_formula::prelude::DBConstraint;
use veripb_parser::error::ParserError;

use crate::prelude::*;

#[derive(Debug)]
pub struct MoveToCoreRule {
    identifier_option: IdentifierOption,
    constraint_ids: Vec<isize>,
}

impl MoveToCoreRule {
    #[inline]
    pub fn new(identifier_option: IdentifierOption, constraint_ids: Vec<isize>) -> Self {
        Self {
            identifier_option,
            constraint_ids,
        }
    }

    #[inline]
    pub fn parse(lex: Lexer<RuleToken>) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let option = IdentifierOption::parse(&mut lex)?;
        let mut rule = MoveToCoreRule::new(option, Vec::new());
        match rule.identifier_option {
            IdentifierOption::Id => {
                let mut lex = lex.morph();
                while let Some(constraint_id) = IntegerToken::parse_optional(&mut lex)? {
                    rule.constraint_ids.push(constraint_id);
                }
            }
            IdentifierOption::Range => {
                let mut lex = lex.morph();
                rule.constraint_ids.push(IntegerToken::parse(&mut lex)?);
                rule.constraint_ids.push(IntegerToken::parse(&mut lex)?);
            }
            IdentifierOption::Specification => {
                return Err(ParserError::token_error(lex.span(), "'id' or 'range'"));
            }
        }

        Ok(rule)
    }
}

impl Rule for MoveToCoreRule {
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        match self.identifier_option {
            IdentifierOption::Id => {
                for index in self.constraint_ids.iter_mut() {
                    *index = database.normalize_id(*index);
                    database.move_to_core(&mut context.propagation_engine, *index as usize)?;
                }
            }
            IdentifierOption::Range => {
                // Normalize constraint IDs to get rid of negative IDs.
                let range = {
                    let lower_bound = database.normalize_id(self.constraint_ids[0]) as usize;
                    let upper_bound = database.normalize_id(self.constraint_ids[1]) as usize;
                    lower_bound..upper_bound
                };

                self.constraint_ids = database
                    .get_undeleted(range)
                    .map(|result_id| result_id.map(|id| id as isize))
                    .collect::<Result<_, CheckingError>>()?;

                // Actually move the constraints between first and second ID in the vector to core.
                for &index in self.constraint_ids.iter() {
                    database.move_to_core(&mut context.propagation_engine, index as usize)?;
                }
            }
            IdentifierOption::Specification => unreachable!(),
        }

        Ok(vec![])
    }

    fn elaborate(
        &self,
        context: &mut Context,
        database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.write("core id");
        for &index in self.constraint_ids.iter() {
            let constraint = database
                .get_entry_usize(index as usize)
                .expect("constraint already accessed");
            elaborator.write(" ");
            elaborator.write(
                &constraint
                    .get_out_id(index as usize)
                    .expect("should have out ID")
                    .to_string(),
            );
        }
        elaborator.writeln(";");

        Ok(())
    }
}
