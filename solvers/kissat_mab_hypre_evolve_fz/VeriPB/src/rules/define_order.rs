use std::rc::Rc;

use logos::Lexer;
use veripb_formula::prelude::*;
use veripb_parser::error::ParserError;
use veripb_propagator::propagation_engine::PropagationEngine;

use crate::{order_context::OrderContext, prelude::*};

#[derive(Debug)]
pub struct DefineOrderRule {
    name: String,
}

impl DefineOrderRule {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    #[inline]
    pub fn parse(lex: Lexer<RuleToken>) -> Result<Self, ParserError> {
        Ok(DefineOrderRule {
            name: OrderName::parse(&mut lex.morph())?,
        })
    }
}

impl Rule for DefineOrderRule {
    #[inline]
    fn compute(
        &mut self,
        _context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        Ok(vec![])
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.write("def_order ");
        elaborator.writeln(&self.name);
        Ok(())
    }

    #[inline]
    fn swaps_database(&self) -> bool {
        true
    }

    #[inline]
    fn get_new_database(&mut self) -> (Database, PropagationEngine) {
        (Database::new(), new_veripb_propagation_engine())
    }

    #[inline]
    fn handle_old_database(
        &mut self,
        context: &mut Context,
        database: Database,
        prop_engine: PropagationEngine,
    ) {
        let mut order_context = OrderContext::new(
            std::mem::take(&mut self.name),
            database,
            prop_engine,
            std::mem::take(&mut context.active_order),
        );
        if let Some(elaborator) = context.elaborator.as_mut() {
            order_context.stored_elaborator_id = elaborator.current_id;
            elaborator.current_id = 0;
        }
        context.subcontexts.push(Subcontext::Order(order_context));
    }
}
