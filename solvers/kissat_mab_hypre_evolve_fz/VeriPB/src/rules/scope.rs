use std::{fmt::Display, rc::Rc};

use veripb_formula::prelude::DBConstraint;

use crate::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum ScopeId {
    LessEqual = 0,
    GreaterEqual = 1,
}

impl Display for ScopeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeId::LessEqual => write!(f, "leq"),
            ScopeId::GreaterEqual => write!(f, "geq"),
        }
    }
}

#[derive(Debug)]
pub struct ScopeRule {
    id: ScopeId,
}

impl ScopeRule {
    pub fn new(id: ScopeId) -> Self {
        Self { id }
    }
}

impl Rule for ScopeRule {
    #[inline]
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        if let Some(Subcontext::Subproof(subproof_context)) = context.subcontexts.last_mut() {
            let premises = if let Some(active_order) = &mut context.active_order {
                match self.id {
                    ScopeId::LessEqual => active_order.get_specification_less_equal(),
                    ScopeId::GreaterEqual => {
                        active_order.get_specification_greater_equal(&subproof_context.witness)
                    }
                }
            } else {
                vec![]
            };

            if subproof_context.current_scope.is_some() {
                return Err(CheckingError::StartScopeWhileOtherScopeOpen);
            }
            subproof_context.scope_start_id = database.len();
            subproof_context.current_scope = Some(self.id);

            Ok(premises)
        } else {
            Err(CheckingError::ScopeOutsideSubproof)
        }
    }

    #[inline]
    fn is_subproof_friendly(&self) -> bool {
        true
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        _database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.write("\tscope ");
        elaborator.writeln(&self.id.to_string());
        Ok(())
    }
}
