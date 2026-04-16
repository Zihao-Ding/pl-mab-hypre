use std::rc::Rc;

use logos::Lexer;
use veripb_formula::prelude::*;
use veripb_parser::error::ParserError;

use crate::{order::ActiveOrder, prelude::*};

#[derive(Debug)]
pub struct LoadOrderRule {
    name: Option<String>,
    lits: Vec<Lit>,
    elaboration_required: bool,
}

impl LoadOrderRule {
    pub fn new(name: Option<String>, lits: Vec<Lit>) -> Self {
        Self {
            name,
            lits,
            elaboration_required: true,
        }
    }

    pub fn parse(lex: Lexer<RuleToken>, context: &mut Context) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let name = OrderName::parse_optional(&mut lex)?;

        let mut lits = Vec::new();
        let mut lex = lex.morph();
        while let Some(lit) = LiteralToken::parse_optional(&mut lex, &mut context.var_names)? {
            lits.push(lit);
        }

        Ok(LoadOrderRule {
            name,
            lits,
            elaboration_required: true,
        })
    }
}

impl Rule for LoadOrderRule {
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        match &self.name {
            Some(name) => match context.orders.get(name) {
                Some(order) => {
                    // Load order.
                    if order.left_vars.len() != self.lits.len() {
                        return Err(CheckingError::OrderWrongNumberVariables(
                            order.left_vars.len(),
                            self.lits.len(),
                        ));
                    }
                    database.move_to_core_all(&mut context.propagation_engine)?;

                    context.active_order = Some(ActiveOrder::new(order, self.lits.clone()));
                }
                None => return Err(CheckingError::OrderNotDefined(name.to_owned())),
            },
            None => {
                // Unload order.
                if context.active_order.is_none() {
                    self.elaboration_required = false;
                }
                context.active_order = None;
            }
        }
        Ok(vec![])
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        if self.elaboration_required {
            let elaborator = context.elaborator.as_mut().unwrap();
            elaborator.write("load_order ");
            if let Some(name) = &self.name {
                elaborator.write(name);
                for &lit in self.lits.iter() {
                    elaborator.write(" ");
                    if lit.is_negated() {
                        elaborator.write("~");
                    };
                    elaborator.write(context.var_names.get_name(lit.get_var()));
                }
            }
            elaborator.writeln(";");
        }
        Ok(())
    }
}
