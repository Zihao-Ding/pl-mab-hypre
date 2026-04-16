use crate::error::VeriPBError;
use crate::misc_tokens::IdentifierOption;
use crate::parser::error::ParseError;
use crate::parser::lexer::{IntegerParseMode, Lexer};
use crate::parser::utils::{GenericTerms, PolToken, Position, Scope, Token};
use crate::proofgoal::ProofgoalID;
use crate::rules::{
    AssumptionRule, Bound, ConclusionResult, ConclusionRule, DefineOrderRule, Deletion,
    DeletionOption, DeletionOrigin, DominanceBasedStrengtheningRule, EndProof, EndSubproof,
    EqualsObjectiveRule, EqualsRule, FailRule, FormulaCheck, HeaderRule, ImpliesRule, Instruction,
    IsDeletedCheck, LoadOrderRule, MoveToCoreRule, ObjectiveUpdateRule, ObjectiveUpdateType,
    ObjectiveValueRule, OrderAuxVariablesRule, OrderDefConstraintRule, OrderDefRule,
    OrderFreshAux1Rule, OrderFreshAux2Rule, OrderFreshRightVariablesRule, OrderLeftVariablesRule,
    OrderProofRule, OrderReflexivityRule, OrderRightVariablesRule, OrderSpecificationRule,
    OrderTransitivityRule, OrderVariablesRule, OutputGuarantee, OutputRule, OutputType, PolRule,
    ProofByContradiction, ProofgoalRule, RUPRule, RedundanceBasedStrengtheningRule, Rule, ScopeId,
    ScopeRule, SetLevelRule, SolutionRule, SolutionRuleOutput, StrengtheningToCoreRule,
    UnimplementedRule,
};
use crate::verifier::Verifier;
use ahash::AHashMap;
use malachite_bigint::{BigInt, Sign};
use memmap2::Mmap;
use num_traits::{identities::Zero, Signed};
use std::rc::Rc;
use veripb_formula::db_constraint::DBConstraint;
use veripb_formula::general_pb_term::GeneralPBTerm;
use veripb_formula::lit::Lit;
use veripb_formula::pb_constraint::{constraint_from_terms_and_coeff_sum, Int};
use veripb_formula::pb_objective::PBObjective;
use veripb_formula::prelude::{GeneralPBConstraint, PBConstraintEnum, PBTerm};
use veripb_formula::substitution::{Substitution, SubstitutionValue};
use veripb_formula::var_type::VarIdx;

type OptionalConstraintPair = (Option<PBConstraintEnum>, Option<PBConstraintEnum>);

type Result<T> = std::result::Result<T, VeriPBError>;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    verifier: &'a mut Verifier,
    label_to_id: AHashMap<String, isize>,
}

impl<'a> Parser<'a> {
    pub fn new(
        mmap: Mmap,
        verifier: &'a mut Verifier,
        formula_labels: AHashMap<String, isize>,
    ) -> Self {
        Self {
            lexer: Lexer::new(mmap),
            verifier,
            label_to_id: formula_labels,
        }
    }

    pub fn parse(&mut self) -> Result<()> {
        if self.verifier.context.args.show_progress {
            println!();
        };
        let token = self.lexer.get_next_token()?;
        self.proof(token)?;
        // let mut token = self.lexer.next()?;
        // loop {
        //     self.proof(token)?;
        //     token = self.optional_opb_whitespace()?;
        //     if token == Token::Eof {
        //         break;
        //     };
        // };
        let token = self.optional_opb_whitespace()?;
        self.eof(token)
    }

    #[inline]
    fn execute_rule(&mut self, rule: Box<dyn Rule>) -> Result<()> {
        if self.verifier.context.args.trace {
            self.lexer.print_trace_since_last();
        }
        self.verifier.execute_rule(self.lexer.get_line(), rule)?;
        if self.verifier.context.args.show_progress {
            let current = self.lexer.get_pos().pos as f32;
            let size = self.lexer.get_len() as f32;
            print!(
                "\x1B[?25lProgress: {:>6.2}%\x1B[?25h\r",
                100. * current / size
            );
        };
        Ok(())
    }

    fn is_space_or_tab(&mut self, token: &Token) -> bool {
        matches!(token, Token::Space(_) | Token::Tab(_))
    }

    fn is_white_space(&mut self, token: &Token) -> bool {
        matches!(
            token,
            Token::Space(_) | Token::Tab(_) | Token::Eol(_) | Token::Comment(_)
        )
    }

    fn is_constraint_id(&mut self, token: &Token) -> Result<bool> {
        match token {
            Token::Integer64(Sign::NoSign, id) if *id != 0 => Ok(true),
            Token::Integer64(Sign::Minus, _) => Ok(true),
            Token::Label(_) => Ok(true),
            Token::Integer64(sign, _) => Err(ParseError::new(
                format!(
                    "The constraint ID {} {}",
                    token,
                    if *sign == Sign::Plus {
                        "may not be prefixed with `+`"
                    } else {
                        "is not a valid constraint ID"
                    }
                ),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
            _ => Ok(false),
        }
    }

    fn is_variable(&mut self, token: &Token, is_aux_allowed: bool) -> Result<bool> {
        match token {
            Token::Identifier(s) if s.len() > 1 => Ok(true),
            Token::Identifier(s) => Err(ParseError::new(
                format!("Expected a variable of length at least two but only found `{s}`"),
                self.lexer.get_pos(),
                1,
                self.lexer.get_mmap(),
            ))?,
            Token::AuxiliaryVariable(_) => Ok(is_aux_allowed),
            _ => Ok(false),
        }
    }

    fn is_any_variable(
        &mut self,
        token: &Token,
        is_var_allowed: bool,
        is_aux_allowed: bool,
    ) -> Result<bool> {
        match token {
            Token::Identifier(s) if s.len() > 1 => Ok(is_var_allowed),
            Token::Identifier(s) if is_var_allowed => Err(ParseError::new(
                format!("Expected a variable of length at least two but only found `{s}`"),
                self.lexer.get_pos(),
                1,
                self.lexer.get_mmap(),
            ))?,
            Token::AuxiliaryVariable(_) => Ok(is_aux_allowed),
            _ => Ok(false),
        }
    }

    fn _is_aux_variable(&mut self, token: &Token) -> bool {
        matches!(token, Token::AuxiliaryVariable(_))
    }

    fn is_literal(&mut self, token: &Token, is_aux_allowed: bool) -> Result<bool> {
        match token {
            Token::Tilde => Ok(true),
            token if self.is_variable(token, is_aux_allowed)? => Ok(true),
            _ => Ok(false),
        }
    }

    fn proof(&mut self, token: Token) -> Result<()> {
        self.header(token)?;
        let token = self.optional_whitespace()?;
        let token = self.proof_lines(token, &Scope::TopLevel)?;
        self.footer(token)
    }

    fn header(&mut self, token: Token) -> Result<()> {
        self.keyword("pseudo-Boolean", token)?;
        self.space(1, true)?;
        let token = self.lexer.get_next_token()?;
        self.keyword("proof", token)?;
        self.space(1, true)?;
        let token = self.lexer.get_next_token()?;
        self.keyword("version", token)?;
        let token = self.space_or_tab(1, false)?;
        self.version(token)?;
        let token = self.space_or_tab(0, false)?;
        self.eol(token)?;
        self.execute_rule(Box::new(HeaderRule))?;
        Ok(())
    }

    fn space(&mut self, expected: usize, strict: bool) -> Result<()> {
        match self.lexer.get_next_token()? {
            Token::Space(actual)
                if if strict {
                    actual == expected
                } else {
                    actual >= expected
                } =>
            {
                Ok(())
            }
            token => Err(ParseError::new(
                format!(
                    "Expected {} {expected} space{} but found {}",
                    if strict { "exactly" } else { "at least" },
                    if expected == 1 { "" } else { "s" },
                    token,
                ),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn space_or_tab(&mut self, expected: usize, strict: bool) -> Result<Token> {
        let mut actual = 0;
        let mut token = self.lexer.get_next_token()?;
        while let Token::Space(count) | Token::Tab(count) = token {
            actual += count;
            token = self.lexer.get_next_token()?;
        }
        if if strict {
            actual == expected
        } else {
            actual >= expected
        } {
            Ok(token)
        } else {
            Err(ParseError::new(
                format!(
                    "Expected {0} {expected} space{1}/character tabulator{1} but found {2}",
                    if strict { "exactly" } else { "at least" },
                    if expected == 1 { "" } else { "s" },
                    token,
                ),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?
        }
    }

    fn eol(&mut self, token: Token) -> Result<()> {
        match token {
            Token::Eol(_) => Ok(()),
            token => Err(ParseError::new(
                format!("Expected newline (`\\r`, `\\n` or `\\r\\n`) but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn whitespace(&mut self, is_mandatory: bool, is_comments_allowed: bool) -> Result<Token> {
        let mut has_whitespace = false;
        let mut token = self.lexer.get_next_token()?;
        loop {
            match token {
                Token::Space(_) | Token::Tab(_) | Token::Eol(_) => has_whitespace = true,
                Token::Comment(_) if is_comments_allowed => has_whitespace = true,
                _ => break,
            }
            token = self.lexer.get_next_token()?;
        }
        // is_mandatory ==> has_whitespace
        if !is_mandatory || has_whitespace {
            Ok(token)
        } else {
            Err(ParseError::new(
                format!("Expected mandatory whitespace but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?
        }
    }

    fn mandatory_whitespace(&mut self) -> Result<Token> {
        self.whitespace(true, true)
    }

    fn optional_whitespace(&mut self) -> Result<Token> {
        self.whitespace(false, true)
    }

    fn optional_opb_whitespace(&mut self) -> Result<Token> {
        self.whitespace(false, false)
    }

    fn keyword(&mut self, keyword: &str, token: Token) -> Result<()> {
        match token {
            Token::Identifier(s) if s == keyword => Ok(()),
            token => Err(ParseError::new(
                format!("Expected `{keyword}` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn version(&mut self, token: Token) -> Result<()> {
        let pos = self.lexer.get_pos();
        let mut len = 0;
        let major = match token {
            Token::Integer64(Sign::NoSign, major) => {
                len += token.str_len();
                major
            }
            token @ Token::Integer64(_, _) => {
                return Err(ParseError::new(
                    format!("The proof version {token} may not be prefixed with a sign"),
                    pos,
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
            token => {
                return Err(ParseError::new(
                    format!("Expected an unsigned integer but found {token}"),
                    pos,
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        match self.lexer.get_next_token()? {
            Token::Dot => len += 1,
            _ => return Err(ParseError::new(
                format!("Expected proof version with major and minor version but only found major version `{major}`"),
                pos,
                len,
                self.lexer.get_mmap(),
            ))?,
        };
        let minor = match self.lexer.get_next_token()? {
            Token::Integer64(Sign::NoSign, minor) => {
                len += token.str_len();
                minor
            },
            token @ Token::Integer64(_, _) => return Err(ParseError::new(
                format!("The minor proof version {token} may not be prefixed with a sign"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
            Token::Space(_) |
            Token::Tab(_) |
            Token::Eol(_) |
            Token::Eof => return Err(ParseError::new(
                format!("Expected proof version with major and minor version but only found `{major}.`"),
                pos,
                len,
                self.lexer.get_mmap(),
            ))?,
            token => return Err(ParseError::new(
                format!("Expected proof version with major and minor version but found `{major}.{}`", token.to_string().get(1..token.str_len() + 1).unwrap_or_default()),
                pos,
                len + token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        };
        if major > 3 || (major == 3 && minor > 0) {
            return Err(ParseError::new(
                format!("The current highest supported proof version is `3.0` but found version `{major}.{minor}`"),
                pos,
                len,
                self.lexer.get_mmap(),
            ))?;
        };
        if major == 2 && minor == 0 {
            return Err(ParseError::new(
                "Switch to 2.0 parser".to_string(),
                // Special position with all zeros to allow to fallback to old parser.
                Position {
                    pos: 0,
                    col: 1,
                    line: 0,
                },
                0,
                self.lexer.get_mmap(),
            ))?;
        };
        if major < 3 {
            return Err(ParseError::new(
                format!("Only proof version `3.0` (and `2.0`) are supported but found version `{major}.{minor}`"),
                pos,
                len,
                self.lexer.get_mmap(),
            ))?;
        };
        // Carefully update this if a major/minor version larger than 255 gets allowed!
        self.verifier.context.major_version = Some(major as u8);
        self.verifier.context.minor_version = Some(minor as u8);
        Ok(())
    }

    fn colon(&mut self, token: Token) -> Result<()> {
        match token {
            Token::Colon => Ok(()),
            token => Err(ParseError::new(
                format!("Expected `:` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn semicolon(&mut self, token: Token) -> Result<()> {
        match token {
            Token::Semicolon => Ok(()),
            token => Err(ParseError::new(
                format!("Expected `;` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn colon_or_semicolon(&mut self, token: Token) -> Result<bool> {
        match token {
            Token::Colon => Ok(false),
            Token::Semicolon => Ok(true),
            token => Err(ParseError::new(
                format!("Expected `:` or `;` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn optional_white_space_semicolon(&mut self) -> Result<()> {
        let token = self.optional_whitespace()?;
        self.semicolon(token)
    }

    fn variable(&mut self, token: Token) -> Result<VarIdx> {
        match token {
            Token::Identifier(variable) => {
                Ok(self.verifier.context.var_names.add_by_name(&variable))
            }
            Token::AuxiliaryVariable(variable) => {
                let var = self.verifier.context.var_names.add_by_name(&variable);
                if let Some(order) = self.verifier.context.active_order.as_ref() {
                    if !order.order.is_auxiliary_variable(var) {
                        Err(ParseError::new(
                            "Expected auxiliary variable valid for the current order".to_string(),
                            self.lexer.get_pos(),
                            variable.len(),
                            self.lexer.get_mmap(),
                        ))?
                    }
                }
                Ok(var)
            }
            _ => unreachable!(),
        }
    }

    fn variables(
        &mut self,
        token: Token,
        is_var_allowed: bool,
        is_aux_allowed: bool,
    ) -> Result<(Token, Vec<VarIdx>)> {
        let mut token = token;
        let mut variables = Vec::new();
        while token != Token::Semicolon {
            match token {
                token if self.is_any_variable(&token, is_var_allowed, is_aux_allowed)? => {
                    variables.push(self.variable(token)?)
                }
                token => {
                    return Err(ParseError::new(
                        format!(
                            "Expected {}{}{} but found {}",
                            if is_var_allowed { "a variable" } else { "" },
                            if is_var_allowed && is_aux_allowed {
                                " or "
                            } else {
                                ""
                            },
                            if is_aux_allowed {
                                "an auxiliary variable"
                            } else {
                                ""
                            },
                            token
                        ),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?
                }
            };
            token = match self.lexer.get_next_token()? {
                token if self.is_white_space(&token) => self.optional_whitespace()?,
                Token::Semicolon => Token::Semicolon,
                token => return Err(ParseError::new(
                    format!("Expected mandatory whitespace (before the next variable) or `;` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?,
            };
        }
        Ok((Token::Semicolon, variables))
    }

    fn literal(&mut self, token: Token, is_aux_allowed: bool) -> Result<Lit> {
        let (token, is_positive) = if Token::Tilde == token {
            (self.lexer.get_next_token()?, false)
        } else {
            (token, true)
        };
        match token {
            token if self.is_variable(&token, is_aux_allowed)? => {
                Ok(Lit::from_var(self.variable(token)?, !is_positive))
            }
            token if !is_positive => Err(ParseError::new(
                format!("Expected a variable after `~` to form a literal but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
            token => Err(ParseError::new(
                format!("Expected a literal but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn literals(
        &mut self,
        token: Token,
        is_aux_allowed: bool,
        is_colon_ending: bool,
        is_reification_ending: bool,
    ) -> Result<(Token, Vec<Lit>)> {
        let mut token = token;
        let mut count = 0;
        let mut literals = Vec::new();
        loop {
            match token {
                token if self.is_literal(&token, is_aux_allowed)? => {
                    literals.push(self.literal(token, is_aux_allowed)?);
                }
                token => {
                    return Err(ParseError::new(
                        format!(
                            "Expected a literal{} but found {}",
                            if is_reification_ending {
                                if count > 1 {
                                    " or `==>`"
                                } else {
                                    ", `<==` or `==>`"
                                }
                            } else if is_colon_ending {
                                ", `:` or `;`"
                            } else {
                                " or `;`"
                            },
                            token
                        ),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?
                }
            };
            count += 1;
            token = match self.lexer.get_next_token()? {
                token if self.is_white_space(&token) => self.optional_whitespace()?,
                Token::Colon if is_colon_ending && !is_reification_ending => Token::Colon,
                Token::Semicolon if !is_reification_ending => Token::Semicolon,
                Token::LeftImplication if is_reification_ending && count == 1 => {
                    Token::LeftImplication
                }
                Token::RightImplication if is_reification_ending => Token::RightImplication,
                token => {
                    return Err(ParseError::new(
                        format!(
                        "Expected mandatory whitespace (before the next literal){} but found {}",
                        if is_reification_ending {
                            if count > 1 {
                                " or `==>`"
                            } else {
                                ", `<==` or `==>`"
                            }
                        } else if is_colon_ending {
                            ", `:` or `;`"
                        } else {
                            " or `;`"
                        },
                        token
                    ),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?
                }
            };
            if if is_reification_ending {
                (token == Token::LeftImplication && count == 1) || token == Token::RightImplication
            } else {
                token == Token::Semicolon || (is_colon_ending && token == Token::Colon)
            } {
                return Ok((token, literals));
            };
        }
    }

    fn objective(
        &mut self,
        token: Token,
        is_colon_ending: bool,
        is_opb: bool,
    ) -> Result<(Token, PBObjective)> {
        let mut is_empty = true;
        let mut token = token;
        let mut coefficient = None;
        let mut terms = Vec::new();
        while let Token::IntegerAny(_, integer) = token {
            coefficient = Some(integer);
            is_empty = false;
            token = self.lexer.get_next_token()?;
            token = match token {
                token if !is_opb && self.is_white_space(&token) => self.optional_whitespace()?,
                token if is_opb && self.is_space_or_tab(&token) => self.space_or_tab(0, false)?,
                Token::Colon if is_colon_ending => break,
                Token::Semicolon => break,
                token => {
                    return Err(ParseError::new(
                        format!(
                            "Expected mandatory {} (before the literal){} or `;` but found {}",
                            if is_opb {
                                "spaces/character tabulators"
                            } else {
                                "whitespace"
                            },
                            if is_colon_ending { ", `:`" } else { "" },
                            token
                        ),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?;
                }
            };
            let literal = match token {
                Token::Colon if is_colon_ending => break,
                Token::Semicolon => break,
                token => self.literal(token, !is_opb)?,
            };
            terms.push(GeneralPBTerm::new(coefficient.unwrap(), literal));
            coefficient = None;
            token = if is_opb {
                self.space_or_tab(0, false)?
            } else {
                self.optional_whitespace()?
            };
        }
        self.lexer.disable_dynamic_integer_parse_mode();
        let objective =
            PBObjective::from_terms(terms, coefficient.unwrap_or_else(BigInt::zero), false);
        match token {
            token if is_empty => Err(ParseError::new(
                format!("Expected a coefficient (signed integer) but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
            Token::Colon if is_colon_ending => Ok((Token::Colon, objective)),
            Token::Semicolon => Ok((Token::Semicolon, objective)),
            token => Err(ParseError::new(
                format!(
                    "Expected mandatory {} (before the next coefficient){} or `;` but found {}",
                    if is_opb {
                        "spaces/character tabulators"
                    } else {
                        "whitespace"
                    },
                    if is_colon_ending { ", `:`" } else { "" },
                    token
                ),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn opb_objective(&mut self, token: Token) -> Result<(Token, Option<PBObjective>)> {
        let (token, objective) = self.objective(token, false, true)?;
        Ok((token, Some(objective)))
    }

    fn reification(&mut self, token: Token) -> Result<(Token, Vec<Lit>, bool)> {
        let (token, literals) = self.literals(token, true, false, true)?;
        let is_right_implication = token == Token::RightImplication;
        let token = self.optional_whitespace()?;
        Ok((token, literals, is_right_implication))
    }

    fn terms_64(
        &mut self,
        mut token: Token,
        is_opb: bool,
        is_eq_allowed: bool,
        mut terms: GenericTerms<i64>,
    ) -> Result<(Token, Option<GenericTerms<i64>>)> {
        loop {
            match token {
                Token::Integer64(sign, coefficient) => {
                    let abs_coefficient = coefficient.abs();
                    if !terms.checked_add_abs_coefficient(&abs_coefficient) {
                        self.lexer.set_integer_parse_mode(IntegerParseMode::Bits128);
                        return Ok((
                            Token::Integer128(sign, i128::from(coefficient)),
                            Some(terms),
                        ));
                    };
                    token = if is_opb {
                        self.space_or_tab(1, false)?
                    } else {
                        self.mandatory_whitespace()?
                    };
                    let literal = self.literal(token, !is_opb)?;
                    terms.add_term(sign, abs_coefficient, literal);
                }
                token => return Ok((token, Some(terms))),
            };
            token = match self.lexer.get_next_token()? {
                token if !is_opb && self.is_white_space(&token) => self.optional_whitespace()?,
                token if is_opb && self.is_space_or_tab(&token) => self.space_or_tab(0, false)?,
                Token::GreaterEqual => return Ok((Token::GreaterEqual, Some(terms))),
                Token::LessEqual => return Ok((Token::LessEqual, Some(terms))),
                Token::Equal if is_eq_allowed => return Ok((Token::Equal, Some(terms))),
                token => return Err(ParseError::new(
                    format!(
                        "Expected mandatory whitespace (before the next coefficient),{} `>=` or `<=` but found {}",
                        if is_eq_allowed {" `=`,"} else {""},
                        token
                    ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?,
            };
        }
    }

    fn terms_128(
        &mut self,
        mut token: Token,
        is_opb: bool,
        is_eq_allowed: bool,
        mut terms: GenericTerms<i128>,
    ) -> Result<(Token, Option<GenericTerms<i128>>)> {
        loop {
            match token {
                Token::Integer128(sign, coefficient) => {
                    let abs_coefficient = coefficient.abs();
                    if !terms.checked_add_abs_coefficient(&abs_coefficient) {
                        self.lexer.set_integer_parse_mode(IntegerParseMode::BitsAny);
                        return Ok((
                            Token::IntegerAny(sign, BigInt::from(coefficient)),
                            Some(terms),
                        ));
                    };
                    token = if is_opb {
                        self.space_or_tab(1, false)?
                    } else {
                        self.mandatory_whitespace()?
                    };
                    let literal = self.literal(token, !is_opb)?;
                    terms.add_term(sign, abs_coefficient, literal);
                }
                token => return Ok((token, Some(terms))),
            };
            token = match self.lexer.get_next_token()? {
                token if !is_opb && self.is_white_space(&token) => self.optional_whitespace()?,
                token if is_opb && self.is_space_or_tab(&token) => self.space_or_tab(0, false)?,
                Token::GreaterEqual => return Ok((Token::GreaterEqual, Some(terms))),
                Token::LessEqual => return Ok((Token::LessEqual, Some(terms))),
                Token::Equal if is_eq_allowed => return Ok((Token::Equal, Some(terms))),
                token => return Err(ParseError::new(
                    format!(
                        "Expected mandatory whitespace (before the next coefficient),{} `>=` or `<=` but found {}",
                        if is_eq_allowed {" `=`,"} else {""},
                        token
                    ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?,
            };
        }
    }

    fn terms_any(
        &mut self,
        mut token: Token,
        is_opb: bool,
        is_eq_allowed: bool,
        mut terms: GenericTerms<BigInt>,
    ) -> Result<(Token, Option<GenericTerms<BigInt>>)> {
        loop {
            match token {
                Token::IntegerAny(sign, coefficient) => {
                    let abs_coefficient = coefficient.abs();
                    terms.checked_add_abs_coefficient(&abs_coefficient);
                    token = if is_opb {
                        self.space_or_tab(1, false)?
                    } else {
                        self.mandatory_whitespace()?
                    };
                    let literal = self.literal(token, !is_opb)?;
                    terms.add_term(sign, abs_coefficient, literal);
                }
                token => return Ok((token, Some(terms))),
            };
            token = match self.lexer.get_next_token()? {
                token if !is_opb && self.is_white_space(&token) => self.optional_whitespace()?,
                token if is_opb && self.is_space_or_tab(&token) => self.space_or_tab(0, false)?,
                Token::GreaterEqual => return Ok((Token::GreaterEqual, Some(terms))),
                Token::LessEqual => return Ok((Token::LessEqual, Some(terms))),
                Token::Equal if is_eq_allowed => return Ok((Token::Equal, Some(terms))),
                token => return Err(ParseError::new(
                    format!(
                        "Expected mandatory whitespace (before the next coefficient),{} `>=` or `<=` but found {}",
                        if is_eq_allowed {" `=`,"} else {""},
                        token
                    ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?,
            };
        }
    }

    fn constraint(
        &mut self,
        token: Token,
        is_opb: bool,
        is_eq_allowed: bool,
    ) -> Result<OptionalConstraintPair> {
        let (mut token, literals, is_right_implication) =
            if !is_opb && self.is_literal(&token, true)? {
                self.reification(token)?
            } else {
                (token, Vec::with_capacity(0), false)
            };
        let mut terms_64: Option<GenericTerms<i64>> = Some(GenericTerms::<i64>::default());
        let mut terms_128: Option<GenericTerms<i128>> = None;
        let mut terms_any: Option<GenericTerms<BigInt>> = None;
        (token, terms_64) = self.terms_64(token, is_opb, is_eq_allowed, terms_64.unwrap())?;
        if let Token::Integer128(_, _) = token {
            (terms_64, (token, terms_128)) = (
                None,
                self.terms_128(
                    token,
                    is_opb,
                    is_eq_allowed,
                    GenericTerms::<i128>::from(terms_64.unwrap()),
                )?,
            );
        };
        if let Token::IntegerAny(_, _) = token {
            if terms_64.is_some() {
                (terms_64, (token, terms_any)) = (
                    None,
                    self.terms_any(
                        token,
                        is_opb,
                        is_eq_allowed,
                        GenericTerms::<BigInt>::from(terms_64.unwrap()),
                    )?,
                );
            } else {
                (terms_128, (token, terms_any)) = (
                    None,
                    self.terms_any(
                        token,
                        is_opb,
                        is_eq_allowed,
                        GenericTerms::<BigInt>::from(terms_128.unwrap()),
                    )?,
                );
            }
        };
        let operator = match token {
            Token::GreaterEqual => Token::GreaterEqual,
            Token::LessEqual => Token::LessEqual,
            Token::Equal if is_eq_allowed => Token::Equal,
            token => {
                return Err(ParseError::new(
                    format!(
                        "Expected {}a coefficient (signed integer),{} `>=` or `<=` but found {}",
                        if !is_opb && literals.is_empty() {
                            "a literal, "
                        } else {
                            ""
                        },
                        if is_eq_allowed { " `=`," } else { "" },
                        token
                    ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        token = if is_opb {
            self.space_or_tab(0, false)?
        } else {
            self.optional_whitespace()?
        };
        match token {
            Token::Integer64(_, _) | Token::Integer128(_, _) | Token::IntegerAny(_, _) => {}
            token => {
                return Err(ParseError::new(
                    format!("Expected a coefficient (signed integer) but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        if let Token::Integer64(sign, degree) = token {
            let mut terms = terms_64.unwrap();
            if terms.checked_add_degree(&degree) {
                terms_64 = Some(terms);
            } else {
                token = Token::Integer128(sign, i128::from(degree));
                (terms_64, terms_128) = (None, Some(GenericTerms::<i128>::from(terms)));
            };
        };
        if let Token::Integer128(sign, degree) = token {
            if terms_64.is_some() {
                (terms_64, terms_128) = (None, Some(GenericTerms::<i128>::from(terms_64.unwrap())));
            };
            let mut terms = terms_128.unwrap();
            if terms.checked_add_degree(&degree) {
                terms_128 = Some(terms);
            } else {
                token = Token::IntegerAny(sign, BigInt::from(degree));
                (terms_128, terms_any) = (None, Some(GenericTerms::<BigInt>::from(terms)));
            };
        };
        if let Token::IntegerAny(_, degree) = token {
            if terms_64.is_some() {
                (terms_64, terms_any) =
                    (None, Some(GenericTerms::<BigInt>::from(terms_64.unwrap())));
            } else if terms_128.is_some() {
                (terms_128, terms_any) =
                    (None, Some(GenericTerms::<BigInt>::from(terms_128.unwrap())));
            };
            let mut terms = terms_any.unwrap();
            terms.checked_add_degree(&degree);
            terms_any = Some(terms);
        };
        if terms_64.is_some() {
            let terms = terms_64.unwrap();
            if terms.is_overflow_safe(&operator, literals.len() as i64, &is_right_implication) {
                terms_64 = Some(terms);
            } else {
                (terms_64, terms_128) = (None, Some(GenericTerms::<i128>::from(terms)));
            };
        };
        if terms_128.is_some() {
            let terms = terms_128.unwrap();
            if terms.is_overflow_safe(&operator, literals.len() as i128, &is_right_implication) {
                terms_128 = Some(terms);
            } else {
                (terms_128, terms_any) = (None, Some(GenericTerms::<BigInt>::from(terms)));
            };
        };
        let result = match () {
            _ if terms_64.is_some() => self.generate_constraints(
                terms_64.unwrap(),
                operator,
                literals,
                is_right_implication,
            ),
            _ if terms_128.is_some() => self.generate_constraints(
                terms_128.unwrap(),
                operator,
                literals,
                is_right_implication,
            ),
            _ => self.generate_constraints(
                terms_any.unwrap(),
                operator,
                literals,
                is_right_implication,
            ),
        };
        self.lexer.disable_dynamic_integer_parse_mode();
        Ok(result)
    }

    fn single_constraint(&mut self, token: Token) -> Result<PBConstraintEnum> {
        let (geq_constraint, leq_constraint) = self.constraint(token, false, false)?;
        Ok(geq_constraint.or(leq_constraint).unwrap())
    }

    fn generate_constraints<N: Int>(
        &mut self,
        terms: GenericTerms<N>,
        operator: Token,
        literals: Vec<Lit>,
        is_right_implication: bool,
    ) -> OptionalConstraintPair
    where
        PBConstraintEnum: From<GeneralPBConstraint<N>>,
    {
        let num_literals = literals.len() as i64;
        let (geq_terms, leq_terms) = match operator {
            Token::GreaterEqual => (Some(terms), None),
            Token::LessEqual => (None, Some(terms)),
            Token::Equal => (Some(terms.clone()), Some(terms)),
            _ => unreachable!(),
        };
        let geq_constraint = if operator != Token::LessEqual {
            let (mut terms, degree, sum) = geq_terms.unwrap().destruct();
            let (terms, degree, sum) = if num_literals == 0 {
                // ... >= ...
                (terms, degree, sum)
            } else if is_right_implication {
                // ... ==> ... >= ...
                for literal in &literals {
                    let mut literal = *literal;
                    literal.negate();
                    terms.push(GeneralPBTerm::new(degree.clone(), literal));
                }
                let new_sum = sum + N::from(num_literals) * &degree;
                (terms, degree, new_sum)
            } else {
                // ... <== ... >= ...
                let new_degree = N::from(1) + &sum - degree;
                for term in terms.iter_mut() {
                    term.negate();
                }
                terms.push(GeneralPBTerm::new(new_degree.clone(), literals[0]));
                let new_sum = sum + &new_degree;
                (terms, new_degree, new_sum)
            };
            Some(constraint_from_terms_and_coeff_sum(terms, degree, sum))
        } else {
            None
        };
        let leq_constraint = if operator != Token::GreaterEqual {
            let (mut terms, degree, sum) = leq_terms.unwrap().destruct();
            let (terms, degree, sum) = if num_literals == 0 {
                // ... <= ...
                for term in &mut terms {
                    term.lit.negate();
                }
                (terms, sum.clone() - degree, sum)
            } else if is_right_implication {
                // ... ==> ... <= ...
                let new_degree = sum.clone() - degree;
                for term in &mut terms {
                    term.negate();
                }
                for literal in &literals {
                    let mut literal = *literal;
                    literal.negate();
                    terms.push(GeneralPBTerm::new(new_degree.clone(), literal));
                }
                let new_sum = sum + N::from(num_literals) * &new_degree;
                (terms, new_degree, new_sum)
            } else {
                // ... <== ... <= ...
                let new_degree = degree + N::from(1);
                terms.push(GeneralPBTerm::new(new_degree.clone(), literals[0]));
                let new_sum = sum + &new_degree;
                (terms, new_degree, new_sum)
            };
            Some(constraint_from_terms_and_coeff_sum(terms, degree, sum))
        } else {
            None
        };
        (geq_constraint, leq_constraint)
    }

    fn constraint_id(&mut self, token: Token) -> Result<isize> {
        match token {
            token if !self.is_constraint_id(&token)? => Err(ParseError::new(
                format!("Expected a constraint ID (label or signed integer) but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
            Token::Integer64(_, id) => Ok(id as isize),
            Token::Label(ref label) => match self.label_to_id.get(label) {
                Some(id) => Ok(*id),
                None => Err(ParseError::new(
                    format!("The label {token} is not assigned to a constraint ID"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?,
            },
            _ => unreachable!(),
        }
    }

    fn constraint_ids(
        &mut self,
        token: Token,
        is_colon_ending: bool,
        is_tilde_allowed: bool,
    ) -> Result<(Token, Vec<isize>)> {
        let mut token = token;
        let mut ids = if is_tilde_allowed {
            vec![0]
        } else {
            Vec::new()
        };
        loop {
            match token {
                token if self.is_constraint_id(&token)? => ids.push(self.constraint_id(token)?),
                Token::Tilde if is_tilde_allowed => ids.push(0),
                token => {
                    return Err(ParseError::new(
                        format!(
                        "Expected a constraint ID ({}label or signed integer) or `;` but found {}",
                        if is_tilde_allowed { "`~`, " } else { "" },
                        token
                    ),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?
                }
            };
            token = match self.lexer.get_next_token()? {
                token if self.is_white_space(&token) => self.optional_whitespace()?,
                Token::Colon if is_colon_ending => Token::Colon,
                Token::Semicolon => Token::Semicolon,
                token => return Err(ParseError::new(
                    format!("Expected mandatory whitespace (before the next constraint ID){} or `;` but found {}", if is_colon_ending {" , `:`"} else {""}, token),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?,
            };
            if token == Token::Semicolon || (is_colon_ending && token == Token::Colon) {
                return Ok((token, ids));
            };
        }
    }

    fn proof_lines(&mut self, token: Token, scope: &Scope) -> Result<Token> {
        let mut token = token;
        loop {
            let label = if let Token::Label(label) = token {
                token = self.mandatory_whitespace()?;
                Some(label)
            } else {
                None
            };
            match token {
                Token::Identifier(ref s) if *scope == Scope::TopLevel && s == "output" => {
                    return Ok(token)
                }
                Token::Identifier(ref s)
                    if (*scope == Scope::Specification || *scope == Scope::Scope) && s == "end" =>
                {
                    return Ok(token)
                }
                Token::Identifier(ref s)
                    if (*scope == Scope::Subproof
                        || *scope == Scope::Proof
                        || *scope == Scope::Proofgoal)
                        && s == "qed" =>
                {
                    return Ok(token)
                }
                Token::Identifier(s) => {
                    self.rule(&s, label.is_some(), scope)?;
                    token = self.optional_whitespace()?;
                    if let Some(label) = label {
                        let id = self.verifier.get_returned_constraint_id().unwrap();
                        self.label_to_id.insert(label, id);
                    };
                }
                token => {
                    let additional_allowed = match (scope, label.is_some()) {
                        (Scope::TopLevel, false) => " or `output`",
                        (Scope::Specification, false) => " or `end`",
                        (Scope::Subproof, false) => ", `scope`, `proofgoal` or `qed`",
                        (Scope::Proof, false) => ", `proofgoal` or `qed`",
                        (Scope::Scope, false) => ", `proofgoal` or `end`",
                        (Scope::Proofgoal, false) => " or `qed`",
                        (_, true) => "",
                    };
                    return Err(ParseError::new(
                        format!(
                            "Expected a {} {}rule name{additional_allowed} but found {}",
                            scope.as_str(),
                            if label.is_some() { "output " } else { "" },
                            token,
                        ),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?;
                }
            };
        }
    }

    fn check_rule_usage(
        &mut self,
        rule: &str,
        is_output_context: bool,
        scope_context: &Scope,
        is_output_rule: bool,
        rule_scope: Scope,
    ) -> Result<bool> {
        if is_output_context && !is_output_rule {
            return Err(ParseError::new(
                format!("The rule `{rule}` cannot be prefixed with a label"),
                self.lexer.get_pos(),
                rule.len(),
                self.lexer.get_mmap(),
            ))?;
        };
        if *scope_context > rule_scope {
            return Err(ParseError::new(
                format!(
                    "The rule `{}` is not allowed in a {} context",
                    rule,
                    scope_context.as_str()
                ),
                self.lexer.get_pos(),
                rule.len(),
                self.lexer.get_mmap(),
            ))?;
        };
        Ok(true)
    }

    fn check_proofgoal_usage(
        &mut self,
        is_output_context: bool,
        scope_context: &Scope,
    ) -> Result<bool> {
        if is_output_context {
            return Err(ParseError::new(
                "A proofgoal cannot be prefixed with a label".to_string(),
                self.lexer.get_pos(),
                9,
                self.lexer.get_mmap(),
            ))?;
        };
        if *scope_context == Scope::Proofgoal
            || *scope_context == Scope::Specification
            || *scope_context == Scope::TopLevel
        {
            return Err(ParseError::new(
                format!(
                    "A proofgoal is not allowed in a {} context",
                    scope_context.as_str()
                ),
                self.lexer.get_pos(),
                9,
                self.lexer.get_mmap(),
            ))?;
        };
        Ok(true)
    }

    fn check_scope_usage(
        &mut self,
        is_output_context: bool,
        scope_context: &Scope,
    ) -> Result<bool> {
        if is_output_context {
            return Err(ParseError::new(
                "A scope cannot be prefixed with a label".to_string(),
                self.lexer.get_pos(),
                5,
                self.lexer.get_mmap(),
            ))?;
        };
        if *scope_context != Scope::Subproof {
            return Err(ParseError::new(
                format!(
                    "A scope is not allowed in a {} context",
                    scope_context.as_str()
                ),
                self.lexer.get_pos(),
                5,
                self.lexer.get_mmap(),
            ))?;
        };
        Ok(true)
    }

    fn rule(&mut self, rule: &str, is_output: bool, scope: &Scope) -> Result<()> {
        match rule {
            // <top_rule>
            "del" if self.check_rule_usage(rule, is_output, scope, false, Scope::TopLevel)? => {
                self.del_rule()
            }
            "delc" if self.check_rule_usage(rule, is_output, scope, false, Scope::TopLevel)? => {
                self.delc_rule()
            }
            "deld" if self.check_rule_usage(rule, is_output, scope, false, Scope::TopLevel)? => {
                self.deld_rule()
            }
            "obju" if self.check_rule_usage(rule, is_output, scope, false, Scope::TopLevel)? => {
                self.obju_rule()
            }
            "load_order"
                if self.check_rule_usage(rule, is_output, scope, false, Scope::TopLevel)? =>
            {
                self.load_order_rule()
            }
            "core" if self.check_rule_usage(rule, is_output, scope, false, Scope::TopLevel)? => {
                self.core_rule()
            }
            "setlvl"
                if self.check_rule_usage(rule, is_output, scope, false, Scope::TopLevel)? =>
            {
                self.setlvl_rule()
            }
            "wiplvl"
                if self.check_rule_usage(rule, is_output, scope, false, Scope::TopLevel)? =>
            {
                self.wiplvl_rule()
            }
            "strengthening_to_core"
                if self.check_rule_usage(rule, is_output, scope, false, Scope::TopLevel)? =>
            {
                self.strengthening_to_core_rule()
            }
            "sol" if self.check_rule_usage(rule, is_output, scope, false, Scope::TopLevel)? => {
                self.sol_rule()
            }
            "def_order"
                if self.check_rule_usage(rule, is_output, scope, false, Scope::TopLevel)? =>
            {
                self.def_order_rule()
            }
            // <top_output_rule>
            "dom" if self.check_rule_usage(rule, is_output, scope, true, Scope::TopLevel)? => {
                self.dom_rule()
            }
            "soli" if self.check_rule_usage(rule, is_output, scope, true, Scope::TopLevel)? => {
                self.soli_rule()
            }
            "obji" if self.check_rule_usage(rule, is_output, scope, true, Scope::TopLevel)? => {
                self.obji_rule()
            }
            "solx" if self.check_rule_usage(rule, is_output, scope, true, Scope::TopLevel)? => {
                self.solx_rule()
            }
            // <specification_output_rule>
            "red"
                if self.check_rule_usage(rule, is_output, scope, true, Scope::Specification)? =>
            {
                self.red_rule()
            }
            // <rule>
            "f" if self.check_rule_usage(rule, is_output, scope, false, Scope::Proofgoal)? => {
                self.f_rule()
            }
            "eobj" if self.check_rule_usage(rule, is_output, scope, false, Scope::Proofgoal)? => {
                self.eobj_rule()
            }
            "eord_def"
                if self.check_rule_usage(rule, is_output, scope, false, Scope::Proofgoal)? =>
            {
                self.eord_def_rule()
            }
            "eord_loaded"
                if self.check_rule_usage(rule, is_output, scope, false, Scope::Proofgoal)? =>
            {
                self.eord_loaded_rule()
            }
            "start_time"
                if self.check_rule_usage(rule, is_output, scope, false, Scope::Proofgoal)? =>
            {
                self.start_time_rule()
            }
            "end_time"
                if self.check_rule_usage(rule, is_output, scope, false, Scope::Proofgoal)? =>
            {
                self.end_time_rule()
            }
            "is_deleted"
                if self.check_rule_usage(rule, is_output, scope, false, Scope::Proofgoal)? =>
            {
                self.is_deleted_rule()
            }
            "fail" if self.check_rule_usage(rule, is_output, scope, false, Scope::Proofgoal)? => {
                self.fail_rule()
            }
            // <output_rule>
            "pol" if self.check_rule_usage(rule, is_output, scope, true, Scope::Proofgoal)? => {
                self.pol_rule()
            }
            "rup" if self.check_rule_usage(rule, is_output, scope, true, Scope::Proofgoal)? => {
                self.rup_rule()
            }
            "pbc" if self.check_rule_usage(rule, is_output, scope, true, Scope::Proofgoal)? => {
                self.pbc_rule()
            }
            "e" if self.check_rule_usage(rule, is_output, scope, true, Scope::Proofgoal)? => {
                self.e_rule()
            }
            "i" if self.check_rule_usage(rule, is_output, scope, true, Scope::Proofgoal)? => {
                self.i_rule()
            }
            "ia" if self.check_rule_usage(rule, is_output, scope, true, Scope::Proofgoal)? => {
                self.ia_rule()
            }
            "a" if self.check_rule_usage(rule, is_output, scope, true, Scope::Proofgoal)? => {
                self.a_rule()
            }
            // <proofgoal>
            "proofgoal" if self.check_proofgoal_usage(is_output, scope)? => self.proofgoal_rule(),
            // <scope>
            "scope" if self.check_scope_usage(is_output, scope)? => self.scope_rule(),
            rule => {
                let additional_allowed = match (scope, is_output) {
                    (Scope::TopLevel, false) => " or `output`",
                    (Scope::Specification, false) => " or `end`",
                    (Scope::Subproof, false) => ", `scope`, `proofgoal` or `qed`",
                    (Scope::Proof, false) => ", `proofgoal` or `qed`",
                    (Scope::Scope, false) => ", `proofgoal` or `end`",
                    (Scope::Proofgoal, false) => " or `qed`",
                    (_, true) => "",
                };
                Err(ParseError::new(
                    format!(
                        "Expected a {} {}rule name{additional_allowed} but found `{rule}`",
                        scope.as_str(),
                        if is_output { "output " } else { "" },
                    ),
                    self.lexer.get_pos(),
                    rule.len(),
                    self.lexer.get_mmap(),
                ))?
            }
        }
    }

    fn witness(&mut self) -> Result<(Token, Substitution)> {
        let mut witness = Substitution::default();
        let mut token = self.optional_whitespace()?;
        loop {
            let variable = match token {
                Token::Semicolon => return Ok((Token::Semicolon, witness)),
                Token::Colon => return Ok((Token::Colon, witness)),
                token if self.is_variable(&token, true)? => self.variable(token)?,
                token => {
                    return Err(ParseError::new(
                        format!("Expected a variable, `:` or `;` but found {token}"),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?
                }
            };
            token = match self.mandatory_whitespace()? {
                Token::MapsTo => self.mandatory_whitespace()?,
                Token::Integer64(Sign::NoSign, 0) => Token::Integer64(Sign::NoSign, 0),
                Token::Integer64(Sign::NoSign, 1) => Token::Integer64(Sign::NoSign, 1),
                token if self.is_literal(&token, true)? => token,
                token => return Err(ParseError::new(
                    format!("Expected `->` (before the assigned value), a literal, `0` or `1` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?,
            };
            let value = match token {
                Token::Integer64(Sign::NoSign, 0) => SubstitutionValue::FALSE,
                Token::Integer64(Sign::NoSign, 1) => SubstitutionValue::TRUE,
                token if self.is_literal(&token, true)? => {
                    SubstitutionValue::lit(self.literal(token, true)?)
                }
                token => {
                    return Err(ParseError::new(
                        format!("Expected a literal, `0` or `1` but found {token}"),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?
                }
            };
            witness.set(variable, value);
            token = match self.lexer.get_next_token()? {
                token if self.is_white_space(&token) => self.optional_whitespace()?,
                Token::Semicolon => return Ok((Token::Semicolon, witness)),
                Token::Colon => return Ok((Token::Colon, witness)),
                token => return Err(ParseError::new(
                    format!("Expected mandatory whitespace (before the next variable), `:` or `;` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?,
            };
        }
    }

    fn subproof(&mut self, rule: &str) -> Result<()> {
        let token = self.optional_whitespace()?;
        self.keyword("subproof", token)?;
        let token = self.mandatory_whitespace()?;
        self.proof_lines(token, &Scope::Subproof)?;
        let optional_hint = self.subproof_qed(rule)?;
        self.execute_rule(Box::new(EndSubproof::new(optional_hint)))?;
        Ok(())
    }

    fn subproof_qed(&mut self, rule: &str) -> Result<Option<isize>> {
        let token = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => self.optional_whitespace()?,
            Token::Colon => Token::Colon,
            Token::Semicolon => Token::Semicolon,
            token => return Err(ParseError::new(
                format!("Expected mandatory whitespace (before the optional `{rule}`), `:` or `;` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        };
        match token {
            Token::Semicolon => Ok(None),
            Token::Colon => self.subproof_hint(),
            Token::Identifier(s) if s == rule => match self.optional_whitespace()? {
                Token::Semicolon => Ok(None),
                Token::Colon => self.subproof_hint(),
                token => Err(ParseError::new(
                    format!("Expected `:` or `;` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?,
            },
            token => Err(ParseError::new(
                format!("Expected the optional `{rule}`, `:` or `;` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn subproof_hint(&mut self) -> Result<Option<isize>> {
        let token = self.optional_whitespace()?;
        let constraint_id = self.constraint_id(token)?;
        self.optional_white_space_semicolon()?;
        Ok(Some(constraint_id))
    }

    fn del_rule(&mut self) -> Result<()> {
        let option = match self.mandatory_whitespace()? {
            Token::Identifier(s) if s == "id" => {
                return self.del_common("del", DeletionOrigin::Unknown)
            }
            Token::Identifier(s) if s == "spec" => {
                self.lexer.enable_dynamic_integer_parse_mode();
                let token = self.mandatory_whitespace()?;
                let constraint = self.single_constraint(token)?;
                DeletionOption::Spec(Rc::new(DBConstraint::from(constraint)))
            }
            Token::Identifier(s) if s == "range" => {
                let token = self.mandatory_whitespace()?;
                let start_id = self.constraint_id(token)?;
                let token = self.mandatory_whitespace()?;
                let end_id = self.constraint_id(token)?;
                DeletionOption::Range((start_id, end_id))
            }
            token => {
                return Err(ParseError::new(
                    format!("Expected `id`, `spec` or `range` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        let token = self.optional_whitespace()?;
        let has_witness = !self.colon_or_semicolon(token)?;
        let (token, witness) = if has_witness {
            self.witness()?
        } else {
            (Token::Semicolon, Substitution::default())
        };
        let has_subproof = has_witness && !self.colon_or_semicolon(token)?;
        self.execute_rule(Box::new(Deletion::new(
            option,
            witness,
            has_subproof,
            DeletionOrigin::Unknown,
        )))?;
        if has_subproof {
            self.subproof("del")
        } else {
            Ok(())
        }
    }

    fn delc_rule(&mut self) -> Result<()> {
        self.del_common("delc", DeletionOrigin::Core)
    }

    fn deld_rule(&mut self) -> Result<()> {
        let constraint_ids = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => {
                let token = self.optional_whitespace()?;
                let (token, constraint_ids) = self.constraint_ids(token, false, false)?;
                self.semicolon(token)?;
                constraint_ids
            },
            Token::Semicolon => Vec::with_capacity(0),
            token => return Err(ParseError::new(
                format!("Expected mandatory whitespace (before the next constraint ID) or `;` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        };
        self.execute_rule(Box::new(Deletion::new(
            DeletionOption::Id(constraint_ids),
            Substitution::default(),
            false,
            DeletionOrigin::Derived,
        )))?;
        Ok(())
    }

    fn del_common(&mut self, rule: &str, origin: DeletionOrigin) -> Result<()> {
        let token = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => self.optional_whitespace()?,
            Token::Colon => Token::Colon,
            Token::Semicolon => Token::Semicolon,
            token => return Err(ParseError::new(
                format!("Expected mandatory whitespace (before the next constraint ID), `:` or `;` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        };
        let (token, constraint_ids) = match token {
            token if self.is_constraint_id(&token)? => self.constraint_ids(token, true, false)?,
            Token::Colon | Token::Semicolon => (token, Vec::with_capacity(0)),
            token => {
                return Err(ParseError::new(
                    format!(
                    "Expected a constraint ID (label or signed integer), `:` or `;` but found {token}"
                ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        let has_witness = !self.colon_or_semicolon(token)?;
        let (token, witness) = if has_witness {
            self.witness()?
        } else {
            (Token::Semicolon, Substitution::default())
        };
        let has_subproof = has_witness && !self.colon_or_semicolon(token)?;
        self.execute_rule(Box::new(Deletion::new(
            DeletionOption::Id(constraint_ids),
            witness,
            has_subproof,
            origin,
        )))?;
        if has_subproof {
            self.subproof(rule)
        } else {
            Ok(())
        }
    }

    fn obju_rule(&mut self) -> Result<()> {
        let update_type = match self.mandatory_whitespace()? {
            Token::Identifier(s) if s == "new" => ObjectiveUpdateType::New,
            Token::Identifier(s) if s == "diff" => ObjectiveUpdateType::Diff,
            token => {
                return Err(ParseError::new(
                    format!("Expected `new` or `diff` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.lexer.enable_dynamic_integer_parse_mode();
        self.lexer.set_integer_parse_mode(IntegerParseMode::BitsAny);
        let token = self.mandatory_whitespace()?;
        let (token, objective) = self.objective(token, true, false)?;
        let has_subproof = !self.colon_or_semicolon(token)?;
        self.execute_rule(Box::new(ObjectiveUpdateRule::new(
            objective,
            update_type,
            has_subproof,
        )))?;
        if has_subproof {
            self.subproof("obju")
        } else {
            Ok(())
        }
    }

    fn load_order_rule(&mut self) -> Result<()> {
        let (optional_name, literals) = self.order_load_literals()?;
        self.execute_rule(Box::new(LoadOrderRule::new(optional_name, literals)))?;
        Ok(())
    }

    fn order_load_literals(&mut self) -> Result<(Option<String>, Vec<Lit>)> {
        let token = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => self.optional_whitespace()?,
            Token::Semicolon => return Ok((None, Vec::with_capacity(0))),
            token => {
                return Err(ParseError::new(
                    format!("Expected mandatory whitespace (before the order name) or `;` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        let name = match token {
            Token::Identifier(name) => Some(name),
            Token::Semicolon => return Ok((None, Vec::with_capacity(0))),
            token => {
                return Err(ParseError::new(
                    format!("Expected a order name but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => {
                let token = self.optional_whitespace()?;
                let (token, literals) = self.literals(token, false, false, false)?;
                self.semicolon(token)?;
                Ok((name, literals))
            }
            Token::Semicolon => Ok((name, Vec::with_capacity(0))),
            token => Err(ParseError::new(
                format!(
                    "Expected mandatory whitespace (before the literals) or `;` but found {token}"
                ),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn core_rule(&mut self) -> Result<()> {
        match self.mandatory_whitespace()? {
            Token::Identifier(s) if s == "id" => {
                let token = match self.lexer.get_next_token()? {
                    token if self.is_white_space(&token) => self.optional_whitespace()?,
                    Token::Semicolon => Token::Semicolon,
                    token => return Err(ParseError::new(
                        format!("Expected mandatory whitespace (before the next constraint ID) or `;` but found {token}"),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?,
                };
                let constraint_ids = match token {
                    token if self.is_constraint_id(&token)? => {
                        let (token, constraint_ids) = self.constraint_ids(token, false, false)?;
                        self.semicolon(token)?;
                        constraint_ids
                    },
                    Token::Semicolon => Vec::with_capacity(0),
                    token => return Err(ParseError::new(
                        format!("Expected a constraint ID (label or signed integer) or `;` but found {token}"),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?,
                };
                self.execute_rule(Box::new(MoveToCoreRule::new(
                    IdentifierOption::Id,
                    constraint_ids,
                )))?;
                Ok(())
            }
            Token::Identifier(s) if s == "range" => {
                let token = self.mandatory_whitespace()?;
                let start_id = self.constraint_id(token)?;
                let token = self.mandatory_whitespace()?;
                let end_id = self.constraint_id(token)?;
                self.optional_white_space_semicolon()?;
                self.execute_rule(Box::new(MoveToCoreRule::new(
                    IdentifierOption::Range,
                    vec![start_id, end_id],
                )))?;
                Ok(())
            }
            token => Err(ParseError::new(
                format!("Expected `id` or `range` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn setlvl_rule(&mut self) -> Result<()> {
        match self.mandatory_whitespace()? {
            Token::Integer64(Sign::NoSign, value) => {
                self.optional_white_space_semicolon()?;
                self.execute_rule(Box::new(SetLevelRule::new(value as usize)))?;
                Ok(())
            }
            token => Err(ParseError::new(
                format!("Expected an unsigned integer but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn wiplvl_rule(&mut self) -> Result<()> {
        match self.mandatory_whitespace()? {
            Token::Integer64(Sign::NoSign, value) => {
                self.optional_white_space_semicolon()?;
                self.execute_rule(Box::new(Deletion::new(
                    DeletionOption::Wipe(value as usize),
                    Substitution::default(),
                    false,
                    DeletionOrigin::Unknown,
                )))?;
                Ok(())
            }
            token => Err(ParseError::new(
                format!("Expected an unsigned integer but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn strengthening_to_core_rule(&mut self) -> Result<()> {
        let enable = match self.mandatory_whitespace()? {
            Token::Identifier(s) if s == "on" => true,
            Token::Identifier(s) if s == "off" => false,
            token => {
                return Err(ParseError::new(
                    format!("Expected `on` or `off` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(StrengtheningToCoreRule::new(enable)))?;
        Ok(())
    }

    fn sol_rule(&mut self) -> Result<()> {
        self.sol_common(SolutionRuleOutput::None)
    }

    fn sol_common(&mut self, rule_output: SolutionRuleOutput) -> Result<()> {
        let is_value_allowed = rule_output != SolutionRuleOutput::Excluding;
        let token = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => self.optional_whitespace()?,
            Token::Colon if is_value_allowed => Token::Colon,
            Token::Semicolon => Token::Semicolon,
            token => {
                return Err(ParseError::new(
                    format!(
                        "Expected mandatory whitespace (before the solution){} or `;` but found {token}",
                        if is_value_allowed { ", `:`" } else { "" }
                    ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        let (token, solution) = match token {
            token if self.is_literal(&token, false)? => self.literals(token, false, true, false)?,
            Token::Colon if is_value_allowed => (Token::Colon, Vec::with_capacity(0)),
            Token::Semicolon => (Token::Semicolon, Vec::with_capacity(0)),
            token => {
                return Err(ParseError::new(
                    format!(
                        "Expected a literal{} or `;` but found {}",
                        if is_value_allowed { ", `:`" } else { "" },
                        token
                    ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.lexer.enable_dynamic_integer_parse_mode();
        self.lexer.set_integer_parse_mode(IntegerParseMode::BitsAny);
        let (token, optional_value) = match token {
            Token::Colon if is_value_allowed => {
                let value = match self.optional_whitespace()? {
                    Token::Integer64(_, value) => BigInt::from(value),
                    Token::Integer128(_, value) => BigInt::from(value),
                    Token::IntegerAny(_, value) => value,
                    token => {
                        return Err(ParseError::new(
                            format!(
                                "Expected an objective value (signed integer) but found {token}"
                            ),
                            self.lexer.get_pos(),
                            token.str_len(),
                            self.lexer.get_mmap(),
                        ))?
                    }
                };
                (self.optional_whitespace()?, Some(value))
            }
            Token::Semicolon => (Token::Semicolon, None),
            token => {
                return Err(ParseError::new(
                    format!(
                        "Expected a literal{} or `;` but found {}",
                        if is_value_allowed { ", `:`" } else { "" },
                        token
                    ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.lexer.disable_dynamic_integer_parse_mode();
        self.semicolon(token)?;
        self.execute_rule(Box::new(SolutionRule::new(
            solution,
            rule_output,
            optional_value,
        )))?;
        Ok(())
    }

    fn def_order_rule(&mut self) -> Result<()> {
        let order_name = match self.mandatory_whitespace()? {
            Token::Identifier(name) => name,
            token => {
                return Err(ParseError::new(
                    format!("Expected an order name but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.execute_rule(Box::new(DefineOrderRule::new(order_name)))?;
        let token = self.mandatory_whitespace()?;
        self.order_vars(token)?;
        let token = self.optional_whitespace()?;
        let token = match token {
            Token::Identifier(ref s) if s == "spec" => {
                self.order_spec(token)?;
                self.optional_whitespace()?
            }
            Token::Identifier(ref s) if s == "def" => token,
            token => {
                return Err(ParseError::new(
                    format!("Expected `spec` or `def` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.order_constraints(false, token)?;
        let token = self.optional_whitespace()?;
        let token = match token {
            Token::Identifier(ref s) if s == "transitivity" => {
                self.order_transitivity(token)?;
                self.optional_whitespace()?
            }
            Token::Identifier(ref s) if s == "reflexivity" => token,
            Token::Identifier(ref s) if s == "end" => token,
            token => {
                return Err(ParseError::new(
                    format!("Expected `transitivity`, `reflexivity` or `end` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        match token {
            Token::Identifier(ref s) if s == "reflexivity" => {
                self.order_reflexivity(token)?;
                let token = self.optional_whitespace()?;
                self.keyword("end", token)?;
            }
            Token::Identifier(ref s) if s == "end" => {}
            token => {
                return Err(ParseError::new(
                    format!("Expected `reflexivity` or `end` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.order_end_environment("def_order")
    }

    fn order_variables_list(&mut self, is_aux: bool) -> Result<Vec<VarIdx>> {
        let token = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => self.optional_whitespace()?,
            Token::Semicolon => return Ok(Vec::with_capacity(0)),
            token => {
                return Err(ParseError::new(
                    format!(
                    "Expected mandatory whitespace (before the variables) or `;` but found {token}"
                ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        match token {
            token if self.is_any_variable(&token, !is_aux, is_aux)? => {
                let (token, variables) = self.variables(token, !is_aux, is_aux)?;
                self.semicolon(token)?;
                Ok(variables)
            }
            Token::Semicolon => Ok(Vec::with_capacity(0)),
            token => Err(ParseError::new(
                format!(
                    "Expected a{} variable or `;` but found {}",
                    if is_aux { "n auxiliary" } else { "" },
                    token
                ),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn order_end_environment(&mut self, environment: &str) -> Result<()> {
        let token = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => self.optional_whitespace()?,
            Token::Semicolon => Token::Semicolon,
            token => return Err(ParseError::new(
                format!("Expected mandatory whitespace (before the optional `{environment}`) or `;` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        };
        match token {
            Token::Semicolon => {}
            Token::Identifier(ref s) if s == environment => {
                self.optional_white_space_semicolon()?;
            }
            token => {
                return Err(ParseError::new(
                    format!("Expected the optional `{environment}` or `;` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.execute_rule(Box::new(EndSubproof::new(None)))?;
        Ok(())
    }

    fn order_proof(&mut self, token: Token) -> Result<()> {
        self.keyword("proof", token)?;
        let token = self.mandatory_whitespace()?;
        self.execute_rule(Box::new(OrderProofRule))?;
        let mut label_to_id = AHashMap::new();
        std::mem::swap(&mut label_to_id, &mut self.label_to_id);
        self.proof_lines(token, &Scope::Proof)?;
        self.label_to_id = label_to_id;
        let token = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => self.optional_whitespace()?,
            Token::Semicolon => Token::Semicolon,
            token => return Err(ParseError::new(
                format!("Expected mandatory whitespace (before the optional `proof`) or `;` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        };
        let token = match token {
            Token::Semicolon => Token::Semicolon,
            Token::Identifier(s) if s == "proof" => self.optional_whitespace()?,
            token => Err(ParseError::new(
                format!("Expected the optional `proof` or `;` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        };
        self.semicolon(token)?;
        self.execute_rule(Box::new(EndSubproof::new(None)))?;
        Ok(())
    }

    fn order_vars(&mut self, token: Token) -> Result<()> {
        self.keyword("vars", token)?;
        let token = self.mandatory_whitespace()?;
        self.execute_rule(Box::new(OrderVariablesRule))?;
        self.keyword("left", token)?;
        let left_variables = self.order_variables_list(false)?;
        self.execute_rule(Box::new(OrderLeftVariablesRule::new(left_variables)))?;
        let token = self.optional_whitespace()?;
        self.keyword("right", token)?;
        let right_variables = self.order_variables_list(false)?;
        self.execute_rule(Box::new(OrderRightVariablesRule::new(right_variables)))?;
        match self.optional_whitespace()? {
            Token::Identifier(ref s) if s == "aux" => {
                let aux_variables = self.order_variables_list(true)?;
                self.execute_rule(Box::new(OrderAuxVariablesRule::new(aux_variables)))?;
                let token = self.optional_whitespace()?;
                self.keyword("end", token)?;
            }
            Token::Identifier(ref s) if s == "end" => {}
            token => {
                return Err(ParseError::new(
                    format!("Expected `aux` or `end` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.order_end_environment("vars")
    }

    fn order_spec(&mut self, token: Token) -> Result<()> {
        self.keyword("spec", token)?;
        let token = self.mandatory_whitespace()?;
        self.execute_rule(Box::new(OrderSpecificationRule))?;
        let mut label_to_id = AHashMap::new();
        std::mem::swap(&mut label_to_id, &mut self.label_to_id);
        self.proof_lines(token, &Scope::Specification)?;
        self.label_to_id = label_to_id;
        self.order_end_environment("spec")
    }

    fn order_constraints(&mut self, is_spec: bool, token: Token) -> Result<()> {
        let keyword = if is_spec { "spec" } else { "def" };
        self.keyword(keyword, token)?;
        self.lexer.enable_dynamic_integer_parse_mode();
        let mut token = self.mandatory_whitespace()?;
        if is_spec {
            self.execute_rule(Box::new(UnimplementedRule::new(
                "eord_def.spec".to_string(),
            )))?;
        } else {
            self.execute_rule(Box::new(OrderDefRule))?;
        };
        loop {
            match token {
                Token::Identifier(ref s) if s == "end" => {
                    return self.order_end_environment(keyword)
                }
                Token::Integer64(_, _) | Token::Integer128(_, _) | Token::IntegerAny(_, _) => {}
                ref token if self.is_literal(token, is_spec)? => {}
                token => {
                    return Err(ParseError::new(
                        format!("Expected a coefficient (signed integer), a literal or `end` but found {token}"),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?
                }
            };
            let constraint = self.single_constraint(token)?;
            self.optional_white_space_semicolon()?;
            if is_spec {
                self.execute_rule(Box::new(UnimplementedRule::new(
                    "eord_def.spec.constraint".to_string(),
                )))?;
            } else {
                self.execute_rule(Box::new(OrderDefConstraintRule::new(constraint)))?;
            };
            self.lexer.enable_dynamic_integer_parse_mode();
            token = self.optional_whitespace()?;
        }
    }

    fn order_transitivity(&mut self, token: Token) -> Result<()> {
        self.keyword("transitivity", token)?;
        let token = self.mandatory_whitespace()?;
        self.execute_rule(Box::new(OrderTransitivityRule))?;
        self.keyword("vars", token)?;
        let token = self.mandatory_whitespace()?;
        self.execute_rule(Box::new(OrderVariablesRule))?;
        self.keyword("fresh_right", token)?;
        let fresh_right_variables = self.order_variables_list(false)?;
        self.execute_rule(Box::new(OrderFreshRightVariablesRule::new(
            fresh_right_variables,
        )))?;
        match self.optional_whitespace()? {
            Token::Identifier(ref s) if s == "fresh_aux_1" => {
                let fresh_aux_1_variables = self.order_variables_list(true)?;
                let token = self.optional_whitespace()?;
                self.execute_rule(Box::new(OrderFreshAux1Rule::new(fresh_aux_1_variables)))?;
                self.keyword("fresh_aux_2", token)?;
                let fresh_aux_2_variables = self.order_variables_list(true)?;
                let token = self.optional_whitespace()?;
                self.execute_rule(Box::new(OrderFreshAux2Rule::new(fresh_aux_2_variables)))?;
                self.keyword("end", token)?;
            }
            Token::Identifier(ref s) if s == "end" => {}
            token => {
                return Err(ParseError::new(
                    format!("Expected `fresh_aux_1` or `end` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.order_end_environment("vars")?;
        let token = self.optional_whitespace()?;
        self.order_proof(token)?;
        let token = self.optional_whitespace()?;
        self.keyword("end", token)?;
        self.order_end_environment("transitivity")
    }

    fn order_reflexivity(&mut self, token: Token) -> Result<()> {
        self.keyword("reflexivity", token)?;
        let token = self.mandatory_whitespace()?;
        self.execute_rule(Box::new(OrderReflexivityRule))?;
        self.order_proof(token)?;
        let token = self.optional_whitespace()?;
        self.keyword("end", token)?;
        self.order_end_environment("reflexivity")
    }

    fn dom_rule(&mut self) -> Result<()> {
        self.lexer.enable_dynamic_integer_parse_mode();
        let token = self.mandatory_whitespace()?;
        let constraint = self.single_constraint(token)?;
        let token = self.optional_whitespace()?;
        if self.colon_or_semicolon(token)? {
            if self.verifier.context.args.show_warnings {
                let pos = self.lexer.get_pos();
                println!(
                    "Warning: A witness must be specified for the dom-rule ending on line {} col {}.",
                    pos.line, pos.col,
                );
            };
            self.execute_rule(Box::new(DominanceBasedStrengtheningRule::new(
                constraint,
                Substitution::default(),
                false,
            )))?;
            return Ok(());
        };
        let (token, witness) = self.witness()?;
        if self.colon_or_semicolon(token)? {
            self.execute_rule(Box::new(DominanceBasedStrengtheningRule::new(
                constraint, witness, false,
            )))?;
            return Ok(());
        };
        self.execute_rule(Box::new(DominanceBasedStrengtheningRule::new(
            constraint, witness, true,
        )))?;
        self.subproof("dom")
    }

    fn soli_rule(&mut self) -> Result<()> {
        self.sol_common(SolutionRuleOutput::Improving)
    }

    fn obji_rule(&mut self) -> Result<()> {
        self.lexer.enable_dynamic_integer_parse_mode();
        self.lexer.set_integer_parse_mode(IntegerParseMode::BitsAny);
        let objective_value = match self.mandatory_whitespace()? {
            Token::IntegerAny(_, value) => value,
            token => {
                return Err(ParseError::new(
                    format!("Expected an objective value (signed integer) but found {token}",),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.lexer.disable_dynamic_integer_parse_mode();
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(ObjectiveValueRule::new(objective_value)))?;
        Ok(())
    }

    fn solx_rule(&mut self) -> Result<()> {
        self.sol_common(SolutionRuleOutput::Excluding)
    }

    fn red_rule(&mut self) -> Result<()> {
        self.lexer.enable_dynamic_integer_parse_mode();
        let token = self.mandatory_whitespace()?;
        let constraint = self.single_constraint(token)?;
        let token = self.optional_whitespace()?;
        if self.colon_or_semicolon(token)? {
            if self.verifier.context.args.show_warnings {
                let pos = self.lexer.get_pos();
                println!(
                    "Warning: A witness must be specified for the red-rule ending on line {} col {}.",
                    pos.line, pos.col,
                );
            };
            self.execute_rule(Box::new(RedundanceBasedStrengtheningRule::new(
                constraint,
                Substitution::default(),
                false,
            )))?;
            return Ok(());
        };
        let (token, witness) = self.witness()?;
        if self.colon_or_semicolon(token)? {
            self.execute_rule(Box::new(RedundanceBasedStrengtheningRule::new(
                constraint, witness, false,
            )))?;
            return Ok(());
        };
        self.execute_rule(Box::new(RedundanceBasedStrengtheningRule::new(
            constraint, witness, true,
        )))?;
        self.subproof("red")
    }

    fn f_rule(&mut self) -> Result<()> {
        match self.mandatory_whitespace()? {
            Token::Integer64(Sign::NoSign, value) => {
                self.optional_white_space_semicolon()?;
                self.execute_rule(Box::new(FormulaCheck::new(value as usize)))?;
                Ok(())
            }
            token => Err(ParseError::new(
                format!("Expected an unsigned integer but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn eobj_rule(&mut self) -> Result<()> {
        self.lexer.enable_dynamic_integer_parse_mode();
        self.lexer.set_integer_parse_mode(IntegerParseMode::BitsAny);
        let token = self.mandatory_whitespace()?;
        let (token, objective) = self.objective(token, false, false)?;
        self.semicolon(token)?;
        self.execute_rule(Box::new(EqualsObjectiveRule::new(objective)))?;
        Ok(())
    }

    fn eord_def_rule(&mut self) -> Result<()> {
        let _order_name = match self.mandatory_whitespace()? {
            Token::Identifier(name) => name,
            token => {
                return Err(ParseError::new(
                    format!("Expected an order name but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        let token = self.mandatory_whitespace()?;
        self.execute_rule(Box::new(UnimplementedRule::new("eord_def".to_string())))?;
        self.order_vars(token)?;
        let token = self.optional_whitespace()?;
        let token = match token {
            Token::Identifier(ref s) if s == "spec" => {
                self.order_constraints(true, token)?;
                self.optional_whitespace()?
            }
            Token::Identifier(ref s) if s == "def" => token,
            token => {
                return Err(ParseError::new(
                    format!("Expected `spec` or `def` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.order_constraints(false, token)?;
        let token = self.optional_whitespace()?;
        self.keyword("end", token)?;
        self.order_end_environment("eord_def")
    }

    fn eord_loaded_rule(&mut self) -> Result<()> {
        let (_optional_name, _literals) = self.order_load_literals()?;
        self.execute_rule(Box::new(UnimplementedRule::new("eord_loaded".to_string())))?;
        Ok(())
    }

    fn start_time_rule(&mut self) -> Result<()> {
        let _timer_name = match self.mandatory_whitespace()? {
            Token::Identifier(name) => name,
            token => {
                return Err(ParseError::new(
                    format!("Expected a timer name but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(UnimplementedRule::new("start_time".to_string())))?;
        Ok(())
    }

    fn end_time_rule(&mut self) -> Result<()> {
        let _timer_name = match self.mandatory_whitespace()? {
            Token::Identifier(name) => name,
            token => {
                return Err(ParseError::new(
                    format!("Expected a timer name but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(UnimplementedRule::new("end_time".to_string())))?;
        Ok(())
    }

    fn is_deleted_rule(&mut self) -> Result<()> {
        self.lexer.enable_dynamic_integer_parse_mode();
        let token = self.mandatory_whitespace()?;
        let constraint = self.single_constraint(token)?;
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(IsDeletedCheck::new(constraint)))?;
        Ok(())
    }

    fn fail_rule(&mut self) -> Result<()> {
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(FailRule))?;
        Ok(())
    }

    fn pol_rule(&mut self) -> Result<()> {
        let mut positive_integer = None;
        let mut variable = None;
        let mut stack_size = 0;
        let mut instructions = Vec::new();
        self.lexer.enable_dynamic_integer_parse_mode();
        let mut token = self.mandatory_whitespace()?;
        loop {
            let operand = match token {
                Token::Plus if stack_size >= 2 => {
                    stack_size -= 1;
                    instructions.push(Instruction::Add);
                    None
                }
                Token::Minus if stack_size >= 2 => {
                    stack_size -= 1;
                    instructions.push(Instruction::LowerRHS(positive_integer.unwrap()));
                    positive_integer = None;
                    None
                }
                Token::Star if stack_size >= 2 => {
                    stack_size -= 1;
                    instructions.push(Instruction::Multiply(positive_integer.unwrap()));
                    positive_integer = None;
                    None
                }
                Token::Identifier(ref s) if stack_size >= 2 && s == "d" => {
                    stack_size -= 1;
                    instructions.push(Instruction::NormalizedFormDivide(positive_integer.unwrap()));
                    positive_integer = None;
                    None
                }
                Token::Identifier(ref s) if stack_size >= 2 && s == "c" => {
                    stack_size -= 1;
                    instructions.push(Instruction::VariableFormDivide(positive_integer.unwrap()));
                    positive_integer = None;
                    None
                }
                Token::Identifier(ref s) if stack_size >= 2 && s == "n" => {
                    stack_size -= 1;
                    instructions.push(Instruction::NormalizedFormMIR(positive_integer.unwrap()));
                    positive_integer = None;
                    None
                }
                Token::Identifier(ref s) if stack_size >= 2 && s == "m" => {
                    stack_size -= 1;
                    instructions.push(Instruction::VariableFormMIR(positive_integer.unwrap()));
                    positive_integer = None;
                    None
                }
                Token::Identifier(ref s) if stack_size >= 1 && s == "s" => {
                    instructions.push(Instruction::Saturate);
                    positive_integer = None;
                    None
                }
                Token::Identifier(ref s) if stack_size >= 2 && s == "w" => {
                    stack_size -= 1;
                    instructions.push(Instruction::Weaken(variable.unwrap()));
                    variable = None;
                    None
                }
                Token::Integer64(Sign::NoSign, id) => {
                    if id == 0 {
                        Err(ParseError::new(
                            format!("The constraint ID or integer {} is not valid", token),
                            self.lexer.get_pos(),
                            token.str_len(),
                            self.lexer.get_mmap(),
                        ))?
                    }
                    Some(PolToken::PositiveIntegerOrID(id as isize))
                }
                token if self.is_constraint_id(&token)? => {
                    Some(PolToken::ConstraintId(self.constraint_id(token)?))
                }
                Token::Integer128(Sign::NoSign, id) if stack_size > 0 && id != 0 => {
                    self.lexer.set_integer_parse_mode(IntegerParseMode::Bits64);
                    Some(PolToken::PositiveInteger(BigInt::from(id)))
                }
                Token::IntegerAny(Sign::NoSign, id) if stack_size > 0 && !id.is_zero() => {
                    self.lexer.set_integer_parse_mode(IntegerParseMode::Bits64);
                    Some(PolToken::PositiveInteger(id))
                }
                Token::Tilde => Some(PolToken::Literal(self.literal(token, true)?)),
                ref token @ (Token::Identifier(ref s) | Token::AuxiliaryVariable(ref s))
                    if s != "d"
                        && s != "c"
                        && s != "n"
                        && s != "m"
                        && s != "s"
                        && s != "w"
                        && self.is_literal(token, true)? =>
                {
                    Some(PolToken::Literal(self.literal(token.clone(), true)?))
                }
                Token::Semicolon if stack_size == 1 => {
                    self.lexer.disable_dynamic_integer_parse_mode();
                    self.execute_rule(Box::new(PolRule::new(instructions)))?;
                    return Ok(());
                }
                token => return Err(self.pol_parse_error(token, stack_size, None))?,
            };
            stack_size += operand.is_some() as usize;
            token = match self.lexer.get_next_token()? {
                token if self.is_white_space(&token) => self.optional_whitespace()?,
                Token::Semicolon if stack_size == 1 => Token::Semicolon,
                token => return Err(ParseError::new(
                    format!("Expected mandatory whitespace (before the next operand or operator){} but found {} (there are {} element{} on the stack)", if stack_size == 1 {" or `;`"} else {""}, token, stack_size, if stack_size == 1 {""} else {"s"}),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?,
            };
            // Lookahead to ensure the right kind of token is present for the next operator (if any).
            match (&token, operand) {
                (_, None) => {}
                (Token::Star, Some(PolToken::PositiveIntegerOrID(integer))) => {
                    positive_integer = Some(BigInt::from(integer))
                }
                (Token::Star, Some(PolToken::PositiveInteger(integer))) => {
                    positive_integer = Some(integer)
                }
                (Token::Identifier(ref s), Some(PolToken::PositiveIntegerOrID(integer)))
                    if s == "d" || s == "c" || s == "m" || s == "n" =>
                {
                    positive_integer = Some(BigInt::from(integer))
                }
                (Token::Identifier(ref s), Some(PolToken::PositiveInteger(integer)))
                    if s == "d" || s == "c" || s == "m" || s == "n" =>
                {
                    positive_integer = Some(integer)
                }
                (Token::Minus, Some(PolToken::PositiveIntegerOrID(integer))) => {
                    positive_integer = Some(BigInt::from(integer))
                }
                (Token::Minus, Some(PolToken::PositiveInteger(integer))) => {
                    positive_integer = Some(integer)
                }
                (Token::Identifier(ref s), Some(PolToken::Literal(literal)))
                    if s == "w" && !literal.is_negated() =>
                {
                    variable = Some(literal.get_var())
                }
                (Token::Star, operand) => {
                    return Err(self.pol_parse_error(token, stack_size, operand))?
                }
                (Token::Identifier(ref s), operand)
                    if s == "c" || s == "d" || s == "m" || s == "n" || s == "w" =>
                {
                    return Err(self.pol_parse_error(token, stack_size, operand))?
                }
                (_, Some(PolToken::ConstraintId(id)))
                | (_, Some(PolToken::PositiveIntegerOrID(id))) => {
                    instructions.push(Instruction::ConstraintId(id))
                }
                (_, Some(PolToken::Literal(literal))) => {
                    instructions.push(Instruction::LiteralAxiom(literal))
                }
                (_, operand) => return Err(self.pol_parse_error(token, stack_size, operand))?,
            };
        }
    }

    fn pol_parse_error(
        &self,
        token: Token,
        stack_size: usize,
        operand: Option<PolToken>,
    ) -> ParseError {
        let allowed = match operand {
            _ if stack_size == 0 => "a constraint ID (label or signed integer) or a literal",
            _ if stack_size == 1 => "a constraint ID (label or signed integer), a literal, a variable, an unsigned positive integer, an unary operator (`s`) or `;`",
            Some(PolToken::PositiveIntegerOrID(_)) => "a constraint ID (label or signed integer), a literal, a variable, an unsigned positive integer, an unary operator (`s`) or a binary operator (`+`, `*`, or `d`)",
            Some(PolToken::PositiveInteger(_)) => "a constraint ID (label or signed integer), a literal, a variable, an unsigned positive integer or a binary operator (`*`, or `d`)",
            Some(PolToken::Literal(literal)) if !literal.is_negated() => "a constraint ID (label or signed integer), a literal, a variable, an unsigned positive integer, an unary operator (`s`) or a binary operator (`+` or `w`)",
            _ => "a constraint ID (label or signed integer), a literal, a variable, an unsigned positive integer, an unary operator (`s`) or a binary operator (`+`)",
        };
        ParseError::new(
            format!(
                "Expected {allowed} but found {} (there are {} element{} on the stack)",
                token,
                stack_size,
                if stack_size == 1 { "" } else { "s" }
            ),
            self.lexer.get_pos(),
            token.str_len(),
            self.lexer.get_mmap(),
        )
    }

    fn rup_rule(&mut self) -> Result<()> {
        self.lexer.enable_dynamic_integer_parse_mode();
        let token = self.mandatory_whitespace()?;
        let constraint = self.single_constraint(token)?;
        let token = self.optional_whitespace()?;
        if self.colon_or_semicolon(token)? {
            self.execute_rule(Box::new(RUPRule::new(constraint, None)))?;
            return Ok(());
        }
        let token = self.optional_whitespace()?;
        if token == Token::Semicolon {
            self.execute_rule(Box::new(RUPRule::new(constraint, None)))?;
            return Ok(());
        };
        let (token, hints) = self.constraint_ids(token, false, true)?;
        self.semicolon(token)?;
        self.execute_rule(Box::new(RUPRule::new(constraint, Some(hints))))?;
        Ok(())
    }

    fn pbc_rule(&mut self) -> Result<()> {
        self.lexer.enable_dynamic_integer_parse_mode();
        let token = self.mandatory_whitespace()?;
        let constraint = self.single_constraint(token)?;
        let token = self.optional_whitespace()?;
        let has_subproof = !self.colon_or_semicolon(token)?;
        self.execute_rule(Box::new(ProofByContradiction::new(
            constraint,
            has_subproof,
        )))?;
        if has_subproof {
            self.subproof("pbc")
        } else {
            Ok(())
        }
    }

    fn e_rule(&mut self) -> Result<()> {
        self.lexer.enable_dynamic_integer_parse_mode();
        let token = self.mandatory_whitespace()?;
        let constraint = self.single_constraint(token)?;
        let token = self.optional_whitespace()?;
        if self.colon_or_semicolon(token)? {
            self.execute_rule(Box::new(EqualsRule::new(constraint, None, false)))?;
            return Ok(());
        }
        let token = self.optional_whitespace()?;
        let hint = self.constraint_id(token)?;
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(EqualsRule::new(constraint, Some(hint), false)))?;
        Ok(())
    }

    fn i_rule(&mut self) -> Result<()> {
        self.i_common(false)
    }

    fn ia_rule(&mut self) -> Result<()> {
        self.i_common(true)
    }

    fn i_common(&mut self, add_result: bool) -> Result<()> {
        self.lexer.enable_dynamic_integer_parse_mode();
        let token = self.mandatory_whitespace()?;
        let constraint = self.single_constraint(token)?;
        let token = self.optional_whitespace()?;
        if self.colon_or_semicolon(token)? {
            self.execute_rule(Box::new(ImpliesRule::new(constraint, None, add_result)))?;
            return Ok(());
        }
        let token = self.optional_whitespace()?;
        let hint = self.constraint_id(token)?;
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(ImpliesRule::new(
            constraint,
            Some(hint),
            add_result,
        )))?;
        Ok(())
    }

    fn a_rule(&mut self) -> Result<()> {
        self.lexer.enable_dynamic_integer_parse_mode();
        let token = self.mandatory_whitespace()?;
        let constraint = self.single_constraint(token)?;
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(AssumptionRule::new(constraint)))?;
        Ok(())
    }

    fn scope_rule(&mut self) -> Result<()> {
        let scope_id = match self.mandatory_whitespace()? {
            Token::Identifier(ref s) if s == "leq" => ScopeId::LessEqual,
            Token::Identifier(ref s) if s == "geq" => ScopeId::GreaterEqual,
            token => {
                return Err(ParseError::new(
                    format!("Expected `leq` or `geq` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        let token = self.mandatory_whitespace()?;
        self.execute_rule(Box::new(ScopeRule::new(scope_id)))?;
        self.proof_lines(token, &Scope::Scope)?;
        let token = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => self.optional_whitespace()?,
            Token::Semicolon => Token::Semicolon,
            token => return Err(ParseError::new(
                format!(
                    "Expected mandatory whitespace (before the optional `scope`) or `;` but found {token}"
                ),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        };
        let token = match token {
            Token::Semicolon => Token::Semicolon,
            Token::Identifier(s) if s == "scope" => self.optional_whitespace()?,
            token => {
                return Err(ParseError::new(
                    format!("Expected the optional `scope` or `;` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.semicolon(token)?;
        self.execute_rule(Box::new(EndSubproof::new(None)))?;
        Ok(())
    }

    fn proofgoal_rule(&mut self) -> Result<()> {
        let (proofgoal, id) = match self.mandatory_whitespace()? {
            token @ Token::ProofgoalID(id) => (token, ProofgoalID::Internal(id)),
            token if self.is_constraint_id(&token)? => (token.clone(), ProofgoalID::Database(self.constraint_id(token)?)),
            token => return Err(ParseError::new(
                format!("Expected a proofgoal ID or a constraint ID (label or signed integer) but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        };
        self.execute_rule(Box::new(ProofgoalRule::new(id)))?;
        let token = self.mandatory_whitespace()?;
        self.proof_lines(token, &Scope::Proofgoal)?;
        let token = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => self.optional_whitespace()?,
            Token::Colon => Token::Colon,
            Token::Semicolon => Token::Semicolon,
            token => {
                return Err(ParseError::new(
                    format!("Expected mandatory whitespace (before the optional {proofgoal}), `:` or `;` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        let optional_hint = match token {
            Token::Semicolon => None,
            Token::Colon => self.subproof_hint()?,
            token if token == proofgoal => match self.optional_whitespace()? {
                Token::Semicolon => None,
                Token::Colon => self.subproof_hint()?,
                token => {
                    return Err(ParseError::new(
                        format!("Expected `:` or `;` but found {token}"),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?
                }
            },
            token => {
                return Err(ParseError::new(
                    format!("Expected the optional {proofgoal}, `:` or `;` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.execute_rule(Box::new(EndSubproof::new(optional_hint)))?;
        Ok(())
    }

    fn footer(&mut self, token: Token) -> Result<()> {
        self.output(token)?;
        let token = self.optional_whitespace()?;
        self.conclusion(token)?;
        let token = self.optional_whitespace()?;
        self.keyword("end", token)?;
        self.space(1, true)?;
        let token = self.lexer.get_next_token()?;
        self.keyword("pseudo-Boolean", token)?;
        self.space(1, true)?;
        let token = self.lexer.get_next_token()?;
        self.keyword("proof", token)?;
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(EndProof))?;
        Ok(())
    }

    fn output(&mut self, token: Token) -> Result<()> {
        self.keyword("output", token)?;
        match self.mandatory_whitespace()? {
            Token::Identifier(s) if s == "NONE" => self.output_guarantee_none(),
            Token::Identifier(s) if s == "DERIVABLE" => self.output_guarantee_other(OutputGuarantee::Derivable),
            Token::Identifier(s) if s == "EQUISATISFIABLE" => self.output_guarantee_other(OutputGuarantee::Equisatisfiable),
            Token::Identifier(s) if s == "EQUIOPTIMAL" => self.output_guarantee_other(OutputGuarantee::Equioptimal),
            // Token::Identifier(s) if s == "EQUIENUMERABLE" => self.output_guarantee_other(OutputGuarantee::Equienumerable),
            token => Err(ParseError::new(
                format!(
                    "Expected an output guarantee (`NONE`, `DERIVABLE`, `EQUISATISFIABLE`, `EQUIOPTIMAL`) but found {token}"
                    // , `EQUIENUMERABLE`
                ),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn output_guarantee_none(&mut self) -> Result<()> {
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(OutputRule::new(
            OutputGuarantee::None,
            OutputType::None,
        )))?;
        Ok(())
    }

    fn output_guarantee_other(&mut self, output_guarantee: OutputGuarantee) -> Result<()> {
        match self.mandatory_whitespace()? {
            Token::Identifier(s) if s == "FILE" => self.output_type_file(output_guarantee),
            Token::Identifier(s) if s == "CONSTRAINTS" => self.output_type_constraints(output_guarantee),
            Token::Identifier(s) if s == "IMPLICIT" => self.output_type_implicit(output_guarantee),
            Token::Identifier(s) if s == "PERMUTATION" => self.output_type_permutation(output_guarantee),
            token => Err(ParseError::new(
                format!("Expected an output type (`FILE`, `CONSTRAINTS`, `IMPLICIT`, `PERMUTATION`) but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn output_type_file(&mut self, output_guarantee: OutputGuarantee) -> Result<()> {
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(OutputRule::new(
            output_guarantee,
            OutputType::File,
        )))?;
        Ok(())
    }

    fn output_type_constraints(&mut self, _output_guarantee: OutputGuarantee) -> Result<()> {
        let mut token = self.mandatory_whitespace()?;
        self.keyword("opb", token)?;
        self.lexer.enable_dynamic_integer_parse_mode();
        token = self.whitespace(true, false)?;
        let mut _objective = None;
        let mut constraints = Vec::new();
        loop {
            match token {
                Token::Star => self.lexer.skip_to_eol()?,
                Token::Identifier(ref s) if s == "min" || s == "max" => {
                    token = self.space_or_tab(0, false)?;
                    self.colon(token)?;
                    self.lexer.enable_dynamic_integer_parse_mode();
                    self.lexer.set_integer_parse_mode(IntegerParseMode::BitsAny);
                    token = self.space_or_tab(0, false)?;
                    (token, _objective) = self.opb_objective(token)?;
                    self.semicolon(token)?;
                    self.lexer.enable_dynamic_integer_parse_mode();
                    token = self.optional_opb_whitespace()?;
                    break;
                },
                Token::Label(_) |
                Token::Integer64(_, _) |
                Token::Integer128(_, _) |
                Token::IntegerAny(_, _) => break,
                Token::Identifier(ref s) if s == "end" => break,
                token => return Err(ParseError::new(
                    format!("Expected an opb comment (starting with `*`), an obp objective (starting with `min` or `max`), an opb constraint (starting with a signed integer or relational operator), a label or `end` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?,
            };
            token = self.optional_opb_whitespace()?;
        }
        loop {
            let has_label = if let Token::Label(_) = token {
                token = self.space_or_tab(1, false)?;
                true
            } else {
                false
            };
            match token {
                Token::Star if !has_label => self.lexer.skip_to_eol()?,
                Token::Integer64(_, _) |
                Token::Integer128(_, _) |
                Token::IntegerAny(_, _) |
                Token::GreaterEqual |
                Token::LessEqual |
                Token::Equal => {
                    let (geq_constraint, leq_constraint) = self.constraint(token, true, !has_label)?;
                    if let Some(constraint) = geq_constraint {
                        constraints.push(constraint);
                    };
                    if let Some(constraint) = leq_constraint {
                        constraints.push(constraint);
                    };
                    self.lexer.enable_dynamic_integer_parse_mode();
                    token = self.space_or_tab(0, false)?;
                    self.semicolon(token)?;
                },
                Token::Identifier(ref s) if !has_label && s == "end" => break,
                token => return Err(ParseError::new(
                    format!(
                        "Expected {}an opb constraint (starting with a signed integer or relational operator){} but found {}",
                        if has_label { "" } else { "an opb comment (starting with `*`), " },
                        if has_label { "" } else { " or `end`" },
                        token
                    ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?,
            };
            token = self.optional_opb_whitespace()?;
        }
        self.lexer.disable_dynamic_integer_parse_mode();
        token = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => self.optional_whitespace()?,
            Token::Semicolon => Token::Semicolon,
            token => {
                return Err(ParseError::new(
                    format!(
                        "Expected mandatory whitespace (before the optional `opb`) or `;` but found {token}"
                    ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?;
            }
        };
        token = match token {
            Token::Identifier(s) if s == "opb" => self.optional_whitespace()?,
            Token::Semicolon => Token::Semicolon,
            token => {
                return Err(ParseError::new(
                    format!("Expected the optional `opb` or `;` but found {token}"),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.semicolon(token)?;
        self.execute_rule(Box::new(UnimplementedRule::new(
            "output.CONSTRAINTS".to_string(),
        )))?;
        Ok(())
    }

    fn output_type_implicit(&mut self, output_guarantee: OutputGuarantee) -> Result<()> {
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(OutputRule::new(
            output_guarantee,
            OutputType::Implicit,
        )))?;
        Ok(())
    }

    fn output_type_permutation(&mut self, _output_guarantee: OutputGuarantee) -> Result<()> {
        let token = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => self.optional_whitespace()?,
            Token::Semicolon => Token::Semicolon,
            token => return Err(ParseError::new(
                format!("Expected mandatory whitespace (before the next constraint ID) or `;` but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        };
        let (token, _constraint_ids) = match token {
            token if self.is_constraint_id(&token)? => self.constraint_ids(token, false, false)?,
            Token::Semicolon => (Token::Semicolon, Vec::with_capacity(0)),
            token => {
                return Err(ParseError::new(
                    format!(
                    "Expected a constraint ID (label or signed integer) or `;` but found {token}"
                ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        self.semicolon(token)?;
        self.execute_rule(Box::new(UnimplementedRule::new(
            "output.PERMUTATION".to_string(),
        )))?;
        Ok(())
    }

    fn conclusion(&mut self, token: Token) -> Result<()> {
        self.keyword("conclusion", token)?;
        match self.mandatory_whitespace()? {
            Token::Identifier(s) if s == "NONE" => self.conclusion_none(),
            Token::Identifier(s) if s == "SAT" => self.conclusion_sat(),
            Token::Identifier(s) if s == "UNSAT" => self.conclusion_unsat(),
            Token::Identifier(s) if s == "BOUNDS" => self.conclusion_bounds(),
            token => Err(ParseError::new(
                format!(
                    "Expected a conclusion (`NONE`, `SAT`, `UNSAT`, `BOUNDS`) but found {token}"
                ),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn conclusion_none(&mut self) -> Result<()> {
        self.optional_white_space_semicolon()?;
        self.execute_rule(Box::new(ConclusionRule::new(
            ConclusionResult::None,
            None,
            None,
            None,
            None,
        )))?;
        Ok(())
    }

    fn conclusion_sat(&mut self) -> Result<()> {
        let token = self.optional_whitespace()?;
        let solution_hint = if self.colon_or_semicolon(token)? {
            None
        } else {
            match self.optional_whitespace()? {
                token if self.is_literal(&token, false)? => {
                    let (token, literals) = self.literals(token, false, false, false)?;
                    self.semicolon(token)?;
                    Some(literals)
                }
                Token::Semicolon => None,
                token => {
                    return Err(ParseError::new(
                        format!("Expected a literal or `;` but found {token}"),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?
                }
            }
        };
        self.execute_rule(Box::new(ConclusionRule::new(
            ConclusionResult::Satisfiable,
            None,
            None,
            None,
            solution_hint,
        )))?;
        Ok(())
    }

    fn conclusion_unsat(&mut self) -> Result<()> {
        let token = self.optional_whitespace()?;
        let constraint_id_hint = if self.colon_or_semicolon(token)? {
            None
        } else {
            let token = self.optional_whitespace()?;
            let constraint_id = self.constraint_id(token)?;
            self.optional_white_space_semicolon()?;
            Some(constraint_id)
        };
        self.execute_rule(Box::new(ConclusionRule::new(
            ConclusionResult::Unsatisfiable,
            None,
            None,
            constraint_id_hint,
            None,
        )))?;
        Ok(())
    }

    fn conclusion_bounds(&mut self) -> Result<()> {
        self.lexer.enable_dynamic_integer_parse_mode();
        self.lexer.set_integer_parse_mode(IntegerParseMode::BitsAny);
        let token = self.mandatory_whitespace()?;
        let lower_bound = Some(self.bound(token, false)?);
        let token = match self.lexer.get_next_token()? {
            token if self.is_white_space(&token) => self.optional_whitespace()?,
            Token::Colon => Token::Colon,
            token => {
                return Err(ParseError::new(
                    format!(
                    "Expected mandatory whitespace (before the upper bound) or `:` but found {token}"
                ),
                    self.lexer.get_pos(),
                    token.str_len(),
                    self.lexer.get_mmap(),
                ))?
            }
        };
        let (token, constraint_id_hint) = if let Token::Colon = token {
            self.lexer.disable_dynamic_integer_parse_mode();
            let token = self.optional_whitespace()?;
            let constraint_id = self.constraint_id(token)?;
            self.lexer.enable_dynamic_integer_parse_mode();
            self.lexer.set_integer_parse_mode(IntegerParseMode::BitsAny);
            (self.mandatory_whitespace()?, Some(constraint_id))
        } else {
            (token, None)
        };
        let upper_bound = Some(self.bound(token, true)?);
        self.lexer.disable_dynamic_integer_parse_mode();
        let token = self.optional_whitespace()?;
        let solution_hint = if self.colon_or_semicolon(token)? {
            None
        } else {
            match self.optional_whitespace()? {
                token if self.is_literal(&token, false)? => {
                    let (token, literals) = self.literals(token, false, false, false)?;
                    self.semicolon(token)?;
                    Some(literals)
                }
                Token::Semicolon => None,
                token => {
                    return Err(ParseError::new(
                        format!("Expected a literal or `;` but found {token}"),
                        self.lexer.get_pos(),
                        token.str_len(),
                        self.lexer.get_mmap(),
                    ))?
                }
            }
        };
        self.execute_rule(Box::new(ConclusionRule::new(
            ConclusionResult::Bounds,
            lower_bound,
            upper_bound,
            constraint_id_hint,
            solution_hint,
        )))?;
        Ok(())
    }

    fn bound(&mut self, token: Token, is_colon_ending: bool) -> Result<Bound> {
        match token {
            Token::IntegerAny(_, coefficient) => Ok(Bound::Bounded(coefficient)),
            Token::Identifier(s) if s == "INF" => Ok(Bound::Unbounded),
            token => Err(ParseError::new(
                format!(
                    "Expected {}a signed integer or `INF` but found {}",
                    if is_colon_ending { "`:`, " } else { "" },
                    token
                ),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }

    fn eof(&mut self, token: Token) -> Result<()> {
        match token {
            Token::Eof => Ok(()),
            token => Err(ParseError::new(
                format!("Expected end of file (EOF) but found {token}"),
                self.lexer.get_pos(),
                token.str_len(),
                self.lexer.get_mmap(),
            ))?,
        }
    }
}
