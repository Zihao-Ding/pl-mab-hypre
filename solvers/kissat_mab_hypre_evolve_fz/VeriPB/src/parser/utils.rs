use std::fmt::Display;

use malachite_bigint::{BigInt, Sign};
use num_traits::Zero;
use veripb_formula::general_pb_term::GeneralPBTerm;
use veripb_formula::lit::Lit;
use veripb_formula::pb_constraint::Int;

#[derive(Debug, PartialEq, PartialOrd)]
pub enum Scope {
    // TopLevel < Specification < Subproof < (Proof < Scope) < Proofgoal
    TopLevel = 0,
    Specification = 1,
    Subproof = 2,
    Proof = 3,
    #[allow(clippy::enum_variant_names)]
    Scope = 4,
    Proofgoal = 5,
}

impl Scope {
    pub fn as_str(&self) -> &str {
        match self {
            Scope::TopLevel => "top level",
            Scope::Specification => "specification",
            Scope::Subproof => "subproof",
            Scope::Proof => "proof",
            Scope::Scope => "scope",
            Scope::Proofgoal => "proofgoal",
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum PolToken {
    ConstraintId(isize),
    PositiveIntegerOrID(isize),
    PositiveInteger(BigInt),
    Literal(Lit),
}

#[derive(Debug, Clone)]
pub struct Position {
    pub pos: usize,
    pub col: usize,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Space(usize),
    Tab(usize),
    Eol(usize),
    Comment(String),
    Identifier(String),
    Integer64(Sign, i64),
    Integer128(Sign, i128),
    IntegerAny(Sign, BigInt),
    Label(String),
    AuxiliaryVariable(String),
    ProofgoalID(usize),
    Colon,
    Semicolon,
    Tilde,
    GreaterEqual,
    LessEqual,
    LeftImplication,
    RightImplication,
    Plus,
    Star,
    MapsTo,
    Minus, // Needed to have `pseudo-Boolean` as three tokens.
    Dot,   // Needed for version number `3.0` as three tokens.
    Equal, // Needed for `=` in OPB files.
    Eof,   // Needed to avoid having options.
}

impl Token {
    pub fn str_len(&self) -> usize {
        match self {
            Token::Space(count) => *count,
            Token::Tab(count) => *count,
            Token::Eol(count) => *count,
            Token::Comment(comment) => comment.len() + 2,
            Token::Identifier(identifier) => identifier.len(),
            Token::Integer64(sign, value) => {
                value.to_string().len()
                    + if (*sign == Sign::Plus) || (*sign == Sign::Minus && *value == 0) {
                        1
                    } else {
                        0
                    }
            }
            Token::Integer128(sign, value) => {
                value.to_string().len()
                    + if (*sign == Sign::Plus) || (*sign == Sign::Minus && *value == 0) {
                        1
                    } else {
                        0
                    }
            }
            Token::IntegerAny(sign, value) => {
                value.to_string().len()
                    + if (*sign == Sign::Plus) || (*sign == Sign::Minus && value.is_zero()) {
                        1
                    } else {
                        0
                    }
            }
            Token::Label(label) => label.len(),
            Token::AuxiliaryVariable(name) => name.len(),
            Token::ProofgoalID(id) => id.to_string().len() + 1,
            Token::Colon => 1,
            Token::Semicolon => 1,
            Token::Tilde => 1,
            Token::GreaterEqual => 2,
            Token::LessEqual => 2,
            Token::LeftImplication => 3,
            Token::RightImplication => 3,
            Token::Plus => 1,
            Token::Star => 1,
            Token::MapsTo => 2,
            Token::Minus => 1,
            Token::Dot => 1,
            Token::Equal => 1,
            Token::Eof => 0,
        }
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Space(count) => {
                write!(f, "{} space{}", count, if *count == 1 { "" } else { "s" })
            }
            Token::Tab(count) => write!(
                f,
                "{} character tabulator{}",
                count,
                if *count == 1 { "" } else { "s" }
            ),
            Token::Eol(_) => write!(f, "newline"),
            Token::Comment(comment) => write!(f, "`%{comment}`"),
            Token::Identifier(identifier) => write!(f, "`{identifier}`"),
            Token::Integer64(sign, value) => write!(
                f,
                "`{}{}`",
                if *sign == Sign::Plus {
                    "+"
                } else if *sign == Sign::Minus && *value == 0 {
                    "-"
                } else {
                    ""
                },
                value
            ),
            Token::Integer128(sign, value) => write!(
                f,
                "`{}{}`",
                if *sign == Sign::Plus {
                    "+"
                } else if *sign == Sign::Minus && *value == 0 {
                    "-"
                } else {
                    ""
                },
                value
            ),
            Token::IntegerAny(sign, value) => write!(
                f,
                "`{}{}`",
                if *sign == Sign::Plus {
                    "+"
                } else if *sign == Sign::Minus && value.is_zero() {
                    "-"
                } else {
                    ""
                },
                value
            ),
            Token::Label(label) => write!(f, "`{label}`"),
            Token::AuxiliaryVariable(name) => write!(f, "`{name}`"),
            Token::ProofgoalID(id) => write!(f, "`#{id}`"),
            Token::Colon => write!(f, "`:`"),
            Token::Semicolon => write!(f, "`;`"),
            Token::Tilde => write!(f, "`~`"),
            Token::GreaterEqual => write!(f, "`>=`"),
            Token::LessEqual => write!(f, "`<=`"),
            Token::LeftImplication => write!(f, "`<==`"),
            Token::RightImplication => write!(f, "`==>`write!(f,"),
            Token::Plus => write!(f, "`+`"),
            Token::Star => write!(f, "`*`"),
            Token::MapsTo => write!(f, "`->`"),
            Token::Minus => write!(f, "`-`"),
            Token::Dot => write!(f, "`.`"),
            Token::Equal => write!(f, "`=`"),
            Token::Eof => write!(f, "end of file (EOF)"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GenericTerms<N: Int> {
    terms: Vec<GeneralPBTerm<N>>,
    degree: N,
    sum: N,
}

impl<N: Int> GenericTerms<N> {
    pub fn checked_add_abs_coefficient(&mut self, abs_coefficient: &N) -> bool {
        match self.sum.checked_add(abs_coefficient) {
            Some(sum) => {
                self.sum = sum;
                true
            }
            None => false,
        }
    }

    pub fn add_term(&mut self, sign: Sign, abs_coefficient: N, mut literal: Lit) {
        if sign == Sign::Minus {
            self.degree += &abs_coefficient;
            literal.negate();
        };
        self.terms
            .push(GeneralPBTerm::new(abs_coefficient, literal));
    }

    pub fn checked_add_degree(&mut self, degree: &N) -> bool {
        match self.degree.checked_add(degree) {
            Some(degree) => {
                self.degree = degree;
                true
            }
            None => false,
        }
    }

    pub fn is_overflow_safe(
        &self,
        operator: &Token,
        num_literals: N,
        is_right_implication: &bool,
    ) -> bool {
        (*operator == Token::LessEqual
            || self.is_overflow_safe_geq(&num_literals, is_right_implication))
            && (*operator == Token::GreaterEqual
                || self.is_overflow_safe_leq(&num_literals, is_right_implication))
    }

    pub fn is_overflow_safe_geq(&self, num_literals: &N, is_right_implication: &bool) -> bool {
        if num_literals.is_zero() {
            // ... >= ...
            return true;
        };
        if *is_right_implication {
            // ... ==> ... >= ...
            self.degree
                .checked_mul(num_literals)
                .is_some_and(|temp| temp.checked_add(&self.sum).is_some())
        } else {
            // ... <== ... >= ...
            self.sum.checked_sub(&self.degree).is_some_and(|temp| {
                temp.checked_add(&N::from(1))
                    .is_some_and(|new_degree| new_degree.checked_add(&self.sum).is_some())
            })
        }
    }

    pub fn is_overflow_safe_leq(&self, num_literals: &N, is_right_implication: &bool) -> bool {
        if num_literals.is_zero() {
            // ... <= ...
            return self.sum.checked_sub(&self.degree).is_some();
        };
        if *is_right_implication {
            // ... ==> ... <= ...
            self.sum
                .checked_sub(&self.degree)
                .is_some_and(|new_degree| {
                    new_degree
                        .checked_mul(num_literals)
                        .is_some_and(|temp| temp.checked_add(&self.sum).is_some())
                })
        } else {
            // ... <== ... <= ...
            self.degree
                .checked_sub(&N::from(1))
                .is_some_and(|new_degree| new_degree.checked_add(&self.sum).is_some())
        }
    }

    pub fn destruct(self) -> (Vec<GeneralPBTerm<N>>, N, N) {
        (self.terms, self.degree, self.sum)
    }
}

impl From<GenericTerms<i64>> for GenericTerms<i128> {
    fn from(terms: GenericTerms<i64>) -> Self {
        GenericTerms::<i128> {
            terms: terms.terms.into_iter().map(|t| t.into()).collect(),
            degree: i128::from(terms.degree),
            sum: i128::from(terms.sum),
        }
    }
}

impl From<GenericTerms<i64>> for GenericTerms<BigInt> {
    fn from(terms: GenericTerms<i64>) -> Self {
        GenericTerms::<BigInt> {
            terms: terms.terms.into_iter().map(|t| t.into()).collect(),
            degree: BigInt::from(terms.degree),
            sum: BigInt::from(terms.sum),
        }
    }
}

impl From<GenericTerms<i128>> for GenericTerms<BigInt> {
    fn from(terms: GenericTerms<i128>) -> Self {
        GenericTerms::<BigInt> {
            terms: terms.terms.into_iter().map(|t| t.into()).collect(),
            degree: BigInt::from(terms.degree),
            sum: BigInt::from(terms.sum),
        }
    }
}
