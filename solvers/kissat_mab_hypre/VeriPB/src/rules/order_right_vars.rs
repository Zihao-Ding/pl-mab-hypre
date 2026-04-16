use logos::Lexer;
use veripb_formula::prelude::*;
use veripb_parser::error::ParserError;

use crate::prelude::*;

#[derive(Debug)]
pub struct OrderRightVariablesRule {
    vars: Vec<VarIdx>,
}

impl OrderRightVariablesRule {
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

impl Rule for OrderRightVariablesRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<std::rc::Rc<DBConstraint>>, CheckingError> {
        if let Some(Subcontext::Order(order_context)) = context.subcontexts.last_mut() {
            order_context.right_vars_defined = true;
            order_context.constructed_order.right_vars = self.vars.clone();

            if order_context.constructed_order.set_variables(
                crate::order_context::OrderVariableKind::Mapped,
                self.vars.as_slice(),
            ) {
                Err(CheckingError::OrderVariablesNonDistinct)
            } else if !order_context
                .constructed_order
                .check_symmetric_left_and_right_variables()
            {
                Err(CheckingError::AsymmetricOrderVariables)
            } else {
                Ok(vec![])
            }
        } else {
            Err(CheckingError::RuleOnlyAllowedInOrderVars)
        }
    }

    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.write("\t\tright");
        for &var in self.vars.iter() {
            elaborator.write(" ");
            elaborator.write(context.var_names.get_name(var));
        }
        elaborator.writeln(";");
        Ok(())
    }
}
