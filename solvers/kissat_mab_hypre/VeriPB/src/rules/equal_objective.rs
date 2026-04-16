use logos::Lexer;
use veripb_formula::prelude::{PBObjective, ToPrettyString};
use veripb_parser::{error::ParserError, opb_parser::parse_opb_objective};

use crate::prelude::*;

#[derive(Debug)]
pub struct EqualsObjectiveRule {
    objective: PBObjective,
}

impl EqualsObjectiveRule {
    pub fn new(objective: PBObjective) -> Self {
        Self { objective }
    }

    pub fn parse(lex: Lexer<RuleToken>, context: &mut Context) -> Result<Self, ParserError> {
        let objective = parse_opb_objective(&mut lex.morph(), &mut context.var_names, false)?;

        Ok(Self { objective })
    }
}

impl Rule for EqualsObjectiveRule {
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<std::rc::Rc<veripb_formula::prelude::DBConstraint>>, CheckingError> {
        if let Some(objective) = &context.objective {
            if *objective == self.objective {
                Ok(vec![])
            } else {
                Err(CheckingError::ObjectivesNotEqual)
            }
        } else {
            Err(CheckingError::EqualObjectiveWithoutObjective)
        }
    }

    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.writeln(&format!(
            "eobj {};",
            self.objective.to_pretty_string(&context.var_names)
        ));
        Ok(())
    }
}
