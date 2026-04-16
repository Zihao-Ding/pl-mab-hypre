use std::rc::Rc;

use logos::Lexer;
use veripb_formula::prelude::*;
use veripb_parser::{error::ParserError, opb_parser::parse_single_constraint};

use crate::prelude::*;

#[derive(Debug)]
pub struct ImpliesRule {
    constraint: Rc<DBConstraint>,
    hint: Option<isize>,
    add_result: bool,
}

impl ImpliesRule {
    pub fn new(constraint: PBConstraintEnum, hint: Option<isize>, add_result: bool) -> Self {
        Self {
            constraint: Rc::new(DBConstraint::from(constraint)),
            hint,
            add_result,
        }
    }

    #[inline]
    pub fn parse(
        lex: Lexer<super::RuleToken>,
        context: &mut Context,
        add_result: bool,
    ) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let (geq_constraint, leq_constraint) =
            parse_single_constraint(&mut lex, &mut context.var_names)?;
        if leq_constraint.is_some() {
            return Err(ParserError::token_error(
                0..lex.span().end,
                "inequality constraint",
            ));
        }
        let hint = IntegerToken::parse_optional(&mut lex.morph())?;
        Ok(ImpliesRule {
            constraint: Rc::new(DBConstraint::from(geq_constraint)),
            hint,
            add_result,
        })
    }
}

impl Rule for ImpliesRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        self.hint = Some(check_implication(
            context,
            database,
            &self.constraint,
            self.hint,
        )?);

        if self.add_result {
            Ok(vec![self.constraint.clone()])
        } else {
            Ok(vec![])
        }
    }

    #[inline]
    fn get_returned_id(&self) -> Option<isize> {
        if self.add_result {
            None
        } else {
            self.hint
        }
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        if self.add_result {
            elaborator.write("ia ");
            elaborator.write(&self.constraint.to_pretty_string(&context.var_names));
            elaborator.write(" : ");
            let premise = database
                .get_entry_usize(self.hint.unwrap() as usize)
                .expect("constraint accessed before");
            elaborator.write(
                &premise
                    .get_out_id(premise.get_some_id())
                    .unwrap()
                    .to_string(),
            );
            elaborator.writeln(";");
        }
        Ok(())
    }

    #[inline]
    fn is_subproof_friendly(&self) -> bool {
        true
    }
}
