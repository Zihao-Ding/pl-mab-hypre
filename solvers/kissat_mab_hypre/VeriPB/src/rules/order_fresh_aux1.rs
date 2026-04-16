use logos::Lexer;
use veripb_formula::prelude::*;
use veripb_parser::error::ParserError;

use crate::prelude::*;

#[derive(Debug)]
pub struct OrderFreshAux1Rule {
    vars: Vec<VarIdx>,
}

impl OrderFreshAux1Rule {
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

impl Rule for OrderFreshAux1Rule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        _database: &mut Database,
    ) -> Result<Vec<std::rc::Rc<DBConstraint>>, CheckingError> {
        if let Some(Subcontext::Transitivity(transitivity_context)) = context.subcontexts.last_mut()
        {
            transitivity_context.fresh_aux_1 = self.vars.clone();
            Ok(vec![])
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
        elaborator.write("\t\tfresh_aux_1");
        for &var in self.vars.iter() {
            elaborator.write(" ");
            elaborator.write(context.var_names.get_name(var));
        }
        elaborator.writeln(";");
        Ok(())
    }
}
