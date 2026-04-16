use std::rc::Rc;

use logos::{Lexer, Logos};
use veripb_formula::prelude::DBConstraint;
use veripb_parser::error::ParserError;

use crate::prelude::*;

#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
enum OptionType {
    #[token("on")]
    On,

    #[token("off")]
    Off,
}

#[derive(Debug)]
pub struct StrengtheningToCoreRule {
    enable: bool,
}

impl StrengtheningToCoreRule {
    #[inline]
    pub fn new(enable: bool) -> Self {
        Self { enable }
    }

    #[inline]
    pub fn parse(lex: Lexer<RuleToken>) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        match lex.next() {
            Some(Ok(OptionType::On)) => Ok(Self::new(true)),
            Some(Ok(OptionType::Off)) => Ok(Self::new(false)),
            _ => Err(ParserError::token_error(lex.span(), "`on` or `off`")),
        }
    }
}

impl Rule for StrengtheningToCoreRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        // Check if strengthening to core mode is actually changed.
        if context.args.show_warnings {
            if self.enable {
                if context.is_strengthening_to_core {
                    println!("Warning: Enabling strengthening to core mode, which was already enabled. Maybe this is not intentional and hints towards an error.");
                }
            } else if !context.is_strengthening_to_core {
                println!("Warning: Disabling strengthening to core mode, which was already disabled. Maybe this is not intentional and hints towards an error.")
            }
        }

        if self.enable {
            database.move_to_core_all(&mut context.propagation_engine)?;
        }

        // Change the strengthening to core mode.
        context.is_strengthening_to_core = self.enable;

        Ok(vec![])
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        context.elaborator.as_mut().unwrap().writeln(&format!(
            "strengthening_to_core {};",
            if self.enable { "on" } else { "off" }
        ));
        Ok(())
    }
}
