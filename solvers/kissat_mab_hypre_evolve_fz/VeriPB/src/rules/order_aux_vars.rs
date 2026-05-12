use logos::Lexer;
use veripb_formula::prelude::*;
use veripb_parser::error::ParserError;

use crate::prelude::*;

#[derive(Debug)]
pub struct OrderAuxVariablesRule {
    vars: Vec<VarIdx>,
}

impl OrderAuxVariablesRule {
    pub fn new(vars: Vec<VarIdx>) -> Self {
        Self { vars }
    }

    pub fn parse(lex: Lexer<RuleToken>, context: &mut Context) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let mut vars = Vec::new();

        while let Some(var) = VariableToken::parse_optional(&mut lex, &mut context.var_names)? {
            vars.push(var);
        }

        Ok(Self { vars })
    }
}

impl Rule for OrderAuxVariablesRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<std::rc::Rc<DBConstraint>>, CheckingError> {
        if let Some(Subcontext::Order(order_context)) = context.subcontexts.last_mut() {
            order_context.aux_vars_defined = true;
            order_context.constructed_order.aux_vars = self.vars.clone();

            if order_context.constructed_order.set_variables(
                crate::order_context::OrderVariableKind::Auxiliary,
                self.vars.as_slice(),
            ) {
                Err(CheckingError::OrderVariablesNonDistinct)
            } else {
                Ok(vec![])
            }
        } else {
            Err(CheckingError::RuleOnlyAllowedInOrderVars)
        }
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.write("\t\taux");
        for &var in self.vars.iter() {
            elaborator.write(" ");
            elaborator.write(context.var_names.get_name(var));
        }
        elaborator.writeln(";");
        Ok(())
    }
}
