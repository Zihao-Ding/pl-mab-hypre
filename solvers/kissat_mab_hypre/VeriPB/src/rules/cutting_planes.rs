use std::{rc::Rc, str::FromStr};

use logos::{Lexer, Logos};
use malachite_bigint::{BigInt, Sign};
use veripb_formula::prelude::*;
use veripb_parser::error::ParserError;

use crate::prelude::*;

use super::{Rule, RuleToken};

// Tokens for postfix notation cutting planes proofs.
#[derive(Debug, Logos, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]")]
enum PolToken {
    // The integer is either used as constraint ID or for division/multiplication.
    #[regex("[+-]?[0-9]+")]
    Integer,

    // OPB variable name.
    #[regex("[a-zA-Z_][_a-zA-Z0-9\\-\\^\\[\\]\\{\\}]+")]
    Var,

    // Negation symbol for a literal.
    #[regex("~[a-zA-Z_][_a-zA-Z0-9\\-\\^\\[\\]\\{\\}]+")]
    NegatedVar,

    // Saturate constraint.
    #[token("s")]
    Saturate,

    // Divide constraint in normalized form by integer.
    #[token("d")]
    NormalizedFormDivide,

    // Weaken constraint by variable.
    #[token("w")]
    Weaken,

    // Multiply constraint by integer.
    #[token("*")]
    Multiply,

    // Add two constraints together.
    #[token("+")]
    Add,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Instruction {
    ConstraintId(isize),
    LiteralAxiom(Lit),
    NormalizedFormDivide(BigInt),
    VariableFormDivide(BigInt),
    NormalizedFormMIR(BigInt),
    VariableFormMIR(BigInt),
    Multiply(BigInt),
    Weaken(VarIdx),
    LowerRHS(BigInt),
    Add,
    Saturate,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PolRule {
    instructions: Vec<Instruction>,
}

impl PolRule {
    pub fn new(instructions: Vec<Instruction>) -> Self {
        Self { instructions }
    }

    #[inline]
    pub fn parse(lex: Lexer<RuleToken>, context: &mut Context) -> Result<Self, ParserError> {
        let mut lex = lex.morph();
        let mut instructions = Vec::new();
        let mut integer_buf: Option<&str> = None;
        let mut var_buffer: Option<VarIdx> = None;

        while let Some(token) = lex.next() {
            if let Some(slice) = integer_buf {
                integer_buf = None;
                match token {
                    Ok(PolToken::NormalizedFormDivide) => {
                        let divisor = BigInt::from_str(slice).unwrap();
                        if divisor.sign() != Sign::Plus {
                            return Err(ParserError::token_error(
                                lex.span(),
                                "positive integer as divisor",
                            ));
                        }
                        instructions.push(Instruction::NormalizedFormDivide(divisor));
                        continue;
                    }
                    Ok(PolToken::Multiply) => {
                        let factor = BigInt::from_str(slice).unwrap();
                        if factor.sign() == Sign::Minus {
                            return Err(ParserError::token_error(
                                lex.span(),
                                "non-negative integer as factor",
                            ));
                        }
                        instructions.push(Instruction::Multiply(factor));
                        continue;
                    }
                    _ => instructions.push(Instruction::ConstraintId(slice.parse().unwrap())),
                }
            } else if token == Ok(PolToken::NormalizedFormDivide) || token == Ok(PolToken::Multiply)
            {
                return Err(ParserError::token_error(
                    lex.span(),
                    "integer before division or multiplication in cutting planes step",
                ));
            }

            if let Some(var_idx) = var_buffer {
                var_buffer = None;
                match token {
                    Ok(PolToken::Weaken) => {
                        instructions.push(Instruction::Weaken(var_idx));
                        continue;
                    }
                    _ => {
                        instructions.push(Instruction::LiteralAxiom(Lit::from_var(var_idx, false)));
                    }
                }
            } else if token == Ok(PolToken::Weaken) {
                return Err(ParserError::token_error(
                    lex.span(),
                    "variable name before weakening rule",
                ));
            }

            match token {
                Ok(PolToken::Integer) => integer_buf = Some(lex.slice()),
                Ok(PolToken::Var) => var_buffer = Some(context.var_names.add_by_name(lex.slice())),
                Ok(PolToken::NegatedVar) => instructions.push(Instruction::LiteralAxiom(
                    Lit::from_var(context.var_names.add_by_name(&lex.slice()[1..]), true),
                )),
                Ok(PolToken::Saturate) => instructions.push(Instruction::Saturate),
                Ok(PolToken::Add) => instructions.push(Instruction::Add),
                Err(_) => {
                    return Err(ParserError::token_error(
                        lex.span(),
                        "integer, literal, '+', '*', 'd', 's', or 'w'",
                    ))
                }
                _ => {}
            }
        }

        if let Some(slice) = integer_buf {
            instructions.push(Instruction::ConstraintId(slice.parse().unwrap()))
        }

        if let Some(var_idx) = var_buffer {
            instructions.push(Instruction::LiteralAxiom(Lit::from_var(var_idx, false)));
        }

        Ok(PolRule { instructions })
    }
}

impl Rule for PolRule {
    fn compute(
        &mut self,
        context: &mut Context,
        database: &mut Database,
    ) -> Result<Vec<Rc<DBConstraint>>, CheckingError> {
        let mut stack: Vec<PBConstraintEnum> = Vec::new();

        // We optimize cutting planes derivations by accumulating successive additions of literal axioms
        // and adding them to the base constraint in one go.
        // Boolean tracking whether the top of the stack is a literal axiom (possibly multiplied with a constant).
        let mut top_is_literal_axiom = false;
        // Vector of terms keeping track of the literal axioms that still need to be added.
        let mut literal_axiom_stash = Vec::new();
        // Vector keeping track of variables that still need to be weakened.
        let mut weakening_stash = Vec::new();

        for instruction in self.instructions.iter_mut() {
            if !literal_axiom_stash.is_empty() {
                // We need to process the literal axiom stash if we encounter an operation that does not extend the
                // current sequence of literal axiom additions.
                let need_to_process_stash = if top_is_literal_axiom {
                    // Multiplying the literal axiom on the top of the stack or adding it extends the current sequence.
                    !matches!(instruction, Instruction::Multiply(_) | Instruction::Add)
                } else {
                    // Only pushing a new literal axiom on the stack extend the current sequence.
                    !matches!(instruction, Instruction::LiteralAxiom(_))
                };
                if need_to_process_stash {
                    let last = if top_is_literal_axiom {
                        // If there is still a literal axiom on top of the stack, we remove it temporarily
                        // in order to carry out the addition on the correct constraint.
                        stack.pop()
                    } else {
                        None
                    };
                    if let Some(first) = stack.last_mut() {
                        // Create constraint from literal axioms to be added.
                        let second = constraint_from_terms(
                            std::mem::take(&mut literal_axiom_stash),
                            0.into(),
                        );
                        // Add constraint containing literal axioms.
                        if let Some(replacement) = first.add(&second) {
                            stack.pop();
                            stack.push(replacement);
                        }
                        // Restore temporarily removed literal axiom.
                        if let Some(last) = last {
                            stack.push(last);
                        }
                    } else {
                        return Err(CheckingError::NotEnoughConstraintsOnStack);
                    }
                }
            }

            if !weakening_stash.is_empty() {
                // We need to process the weakening stash if we have no additional weakening step.
                if !matches!(instruction, Instruction::Weaken(_)) {
                    if let Some(constraint) = stack.last_mut() {
                        if let Some(replacement) =
                            constraint.weaken_all(std::mem::take(&mut weakening_stash))
                        {
                            stack.pop();
                            stack.push(replacement);
                        }
                    } else {
                        return Err(CheckingError::NotEnoughConstraintsOnStack);
                    }
                }
            }

            match instruction {
                Instruction::ConstraintId(index) => {
                    *index = database.normalize_id(*index);
                    let constraint = database.get_entry_usize(*index as usize)?;
                    if context.only_core && !constraint.is_core_constraint_id(*index as usize) {
                        return Err(CheckingError::CoreSubproofUsingNonCoreConstraint(*index));
                    }
                    stack.push(constraint.constraint.clone());
                    top_is_literal_axiom = false;
                }
                Instruction::LiteralAxiom(lit) => {
                    stack.push(Cardinality::from_lits(vec![*lit], 0).into());
                    top_is_literal_axiom = true;
                }
                Instruction::Saturate => {
                    match stack.last_mut() {
                        Some(constraint) => constraint.saturate(),
                        None => return Err(CheckingError::NotEnoughConstraintsOnStack),
                    }
                    top_is_literal_axiom = false;
                }
                Instruction::Weaken(var_idx) => {
                    weakening_stash.push(*var_idx);
                    top_is_literal_axiom = false;
                }
                Instruction::NormalizedFormDivide(divisor) => {
                    match stack.last_mut() {
                        Some(constraint) => constraint.normalized_form_div(divisor),
                        None => return Err(CheckingError::NotEnoughConstraintsOnStack),
                    }
                    top_is_literal_axiom = false;
                }
                Instruction::VariableFormDivide(divisor) => {
                    match stack.last_mut() {
                        Some(constraint) => {
                            if let Some(replacement) = constraint.variable_form_div(divisor) {
                                stack.pop();
                                stack.push(replacement);
                            }
                        }
                        None => return Err(CheckingError::NotEnoughConstraintsOnStack),
                    }
                    top_is_literal_axiom = false;
                }
                Instruction::NormalizedFormMIR(divisor) => {
                    match stack.last_mut() {
                        Some(constraint) => {
                            if let Some(replacement) = constraint.normalized_form_mir(divisor) {
                                stack.pop();
                                stack.push(replacement);
                            }
                        }
                        None => return Err(CheckingError::NotEnoughConstraintsOnStack),
                    }
                    top_is_literal_axiom = false;
                }
                Instruction::VariableFormMIR(divisor) => {
                    match stack.last_mut() {
                        Some(constraint) => {
                            if let Some(replacement) = constraint.variable_form_mir(divisor) {
                                stack.pop();
                                stack.push(replacement);
                            }
                        }
                        None => return Err(CheckingError::NotEnoughConstraintsOnStack),
                    }
                    top_is_literal_axiom = false;
                }
                Instruction::Multiply(factor) => match stack.last_mut() {
                    Some(constraint) => {
                        if let Some(replacement) = constraint.multiply(factor) {
                            stack.pop();
                            stack.push(replacement);
                        }
                    }
                    None => return Err(CheckingError::NotEnoughConstraintsOnStack),
                },
                Instruction::Add => {
                    if let Some(second) = stack.pop() {
                        if !top_is_literal_axiom {
                            // The top is not a literal axiom, so we just add it.
                            if let Some(first) = stack.last_mut() {
                                if let Some(replacement) = first.add(&second) {
                                    stack.pop();
                                    stack.push(replacement);
                                }
                            } else {
                                return Err(CheckingError::NotEnoughConstraintsOnStack);
                            }
                        } else {
                            // Store the literal axiom for adding it later.
                            if let Some(term) = second.get_term(0) {
                                literal_axiom_stash.push(term);
                            }
                            top_is_literal_axiom = false;
                        }
                    } else {
                        return Err(CheckingError::NotEnoughConstraintsOnStack);
                    }
                }
                Instruction::LowerRHS(amount) => match stack.last_mut() {
                    Some(constraint) => {
                        if let Some(replacement) = constraint.lower_rhs(amount) {
                            stack.pop();
                            stack.push(replacement);
                        }
                    }
                    None => return Err(CheckingError::NotEnoughConstraintsOnStack),
                },
            }
        }

        if !literal_axiom_stash.is_empty() {
            // Process the remaining literal axioms that still need to be added.
            if let Some(first) = stack.last_mut() {
                let second =
                    constraint_from_terms(std::mem::take(&mut literal_axiom_stash), 0.into());
                if let Some(replacement) = first.add(&second) {
                    stack.pop();
                    stack.push(replacement);
                }
            } else {
                return Err(CheckingError::NotEnoughConstraintsOnStack);
            }
        }

        if !weakening_stash.is_empty() {
            // Process remaining weakening steps.
            if let Some(constraint) = stack.last_mut() {
                if let Some(replacement) =
                    constraint.weaken_all(std::mem::take(&mut weakening_stash))
                {
                    stack.pop();
                    stack.push(replacement);
                }
            } else {
                return Err(CheckingError::NotEnoughConstraintsOnStack);
            }
        }

        if stack.len() != 1 {
            return Err(CheckingError::StackNotOne(stack.len()));
        }

        Ok(vec![Rc::new(DBConstraint::from(
            stack.pop().unwrap().into_smallest_type(),
        ))])
    }

    #[inline]
    fn elaborate(
        &self,
        context: &mut Context,
        database: &Database,
    ) -> Result<(), ElaborationError> {
        let elaborator = context.elaborator.as_mut().unwrap();
        elaborator.write("pol");
        for instruction in self.instructions.iter() {
            elaborator.write(" ");
            match instruction {
                Instruction::ConstraintId(index) => {
                    let constraint = database
                        .get_entry_usize(*index as usize)
                        .expect("constraint at this ID was successfully accessed before");
                    elaborator.write(
                        &constraint
                            .get_out_id(*index as usize)
                            .expect("constraint should have output ID")
                            .to_string(),
                    );
                }
                Instruction::LiteralAxiom(lit) => {
                    elaborator.write(&lit.to_pretty_string(&context.var_names))
                }
                Instruction::NormalizedFormDivide(big_int) => {
                    elaborator.write(&big_int.to_string());
                    elaborator.write(" d");
                }
                Instruction::VariableFormDivide(big_int) => {
                    elaborator.write(&big_int.to_string());
                    elaborator.write(" c");
                }
                Instruction::NormalizedFormMIR(big_int) => {
                    elaborator.write(&big_int.to_string());
                    elaborator.write(" n");
                }
                Instruction::VariableFormMIR(big_int) => {
                    elaborator.write(&big_int.to_string());
                    elaborator.write(" m");
                }
                Instruction::Multiply(big_int) => {
                    elaborator.write(&big_int.to_string());
                    elaborator.write(" *");
                }
                Instruction::Weaken(var) => {
                    elaborator.write(&var.to_pretty_string(&context.var_names));
                    elaborator.write(" w");
                }
                Instruction::Add => elaborator.write("+"),
                Instruction::Saturate => elaborator.write("s"),
                Instruction::LowerRHS(amount) => {
                    elaborator.write(&amount.to_string());
                    elaborator.write(" -");
                }
            }
        }
        elaborator.writeln(";");
        Ok(())
    }

    #[inline]
    fn is_subproof_friendly(&self) -> bool {
        true
    }
}
