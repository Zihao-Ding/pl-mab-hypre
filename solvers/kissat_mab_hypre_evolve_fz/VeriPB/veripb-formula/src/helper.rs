//! Some general helper functions used inside this crate.

use std::{iter::Peekable, slice::Iter};

use malachite_bigint::BigInt;
use num_traits::One;

use crate::prelude::*;

#[derive(Debug, Clone, Copy)]
pub enum ConstraintPropagationResult {
    NoPropagation,
    Propagated,
    Conflict,
}

/// Merge [`Vec<Lit>`] literals and [`Vec<PBTerm`>] to literals.
///
/// This function immediately returns if merging the term in the [`Vec<Lit>`] is not possible due to the coefficient being larger than 1.
pub fn merge_in_situ_lits(
    clause_term: &mut Peekable<Iter<'_, Lit>>,
    summand_term: &mut Peekable<Iter<'_, impl PBTerm>>,
    resulting_lits: &mut Vec<Lit>,
    cancel: &mut i64,
) {
    loop {
        match (clause_term.peek(), summand_term.peek()) {
            (None, None) => break,
            (None, Some(term)) => {
                if !term.get_coeff().is_one() {
                    break;
                }
                resulting_lits.push(term.get_lit());
                summand_term.next();
            }
            (Some(&lit), None) => {
                resulting_lits.push(*lit);
                clause_term.next();
            }
            (Some(&lit), Some(term)) => match lit.get_var().cmp(&term.get_lit().get_var()) {
                std::cmp::Ordering::Less => {
                    resulting_lits.push(*lit);
                    clause_term.next();
                }
                std::cmp::Ordering::Greater => {
                    if !term.get_coeff().is_one() {
                        break;
                    }
                    resulting_lits.push(term.get_lit());
                    summand_term.next();
                }
                std::cmp::Ordering::Equal => {
                    match (lit.is_negated(), term.get_lit().is_negated()) {
                        (false, false) | (true, true) => break,
                        _ => {
                            if *term.get_coeff() == From::from(2) {
                                resulting_lits.push(term.get_lit());
                            } else if !term.get_coeff().is_one() {
                                break;
                            }
                            *cancel += 1;
                            clause_term.next();
                            summand_term.next();
                        }
                    }
                }
            },
        }
    }
}

/// Merge [`Vec<Lit>`] literals and [`Vec<PBTerm`>] to [`Vec<GeneralPBTerm<N>>`].
///
/// This function immediately returns if merging the term in the [`Vec<GeneralPBTerm<N>>`] is not possible due to the coefficient being larger than [`N::MAX`].
pub fn merge_from_lits<N: Int>(
    mut clause_term: Peekable<Iter<'_, Lit>>,
    mut summand_term: Peekable<Iter<'_, impl PBTerm>>,
    resulting_terms: &mut Vec<GeneralPBTerm<N>>,
    cancel: &mut N,
) {
    loop {
        match (clause_term.peek(), summand_term.peek()) {
            (None, None) => break,
            (None, Some(&term)) => {
                resulting_terms.push(GeneralPBTerm::new(
                    term.get_coeff().to_owned().into().try_into().ok().unwrap(),
                    term.get_lit(),
                ));
                summand_term.next();
            }
            (Some(&lit), None) => {
                resulting_terms.push(GeneralPBTerm::new(N::one(), *lit));
                clause_term.next();
            }
            (Some(&lit), Some(&term)) => match lit.get_var().cmp(&term.get_lit().get_var()) {
                std::cmp::Ordering::Less => {
                    resulting_terms.push(GeneralPBTerm::new(N::one(), *lit));
                    clause_term.next();
                }
                std::cmp::Ordering::Greater => {
                    resulting_terms.push(GeneralPBTerm::new(
                        term.get_coeff().to_owned().into().try_into().ok().unwrap(),
                        term.get_lit(),
                    ));
                    summand_term.next();
                }
                std::cmp::Ordering::Equal => {
                    let term = GeneralPBTerm::new(
                        Into::<BigInt>::into(term.get_coeff().to_owned())
                            .try_into()
                            .ok()
                            .unwrap(),
                        term.get_lit(),
                    );
                    resulting_terms.push(GeneralPBTerm::new(N::one(), *lit));
                    *cancel += resulting_terms.last_mut().unwrap().add_with(term);
                    if resulting_terms.last().unwrap().coeff.is_zero() {
                        resulting_terms.pop();
                    }

                    clause_term.next();
                    summand_term.next();
                }
            },
        }
    }
}
