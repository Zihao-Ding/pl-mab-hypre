//! Implementation of [`PBConstraint`] for general pseudo-Boolean constraints.

use std::fmt::Debug;
use std::panic;
use std::{any::TypeId, cmp::Ordering};

use malachite_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, Zero};

use crate::pb_constraint::WEAKEN_ALL_THRESHOLD;
use crate::prelude::*;
use crate::substitution::Substitutable;

/// A general pseudo-Boolean constraint can have any integer as coefficients and and right-hand side.
///
/// For efficiency reasons the constraint has a generic parameter that defines the integer type used. Currently the integer types [`i64`], [`i128`], and [`BigInt`] are supported.
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub struct GeneralPBConstraint<N>
where
    N: Int,
{
    terms: Vec<GeneralPBTerm<N>>,
    coeff_sum: N,
    degree: N,
}

impl<N> GeneralPBConstraint<N>
where
    N: Int,
{
    /// Delete the term containing the [`VarIdx`] `var_idx` from the [`GeneralPBConstraint`] constraint.
    #[inline]
    fn delete_term(&mut self, var_idx: VarIdx) -> N {
        if let Ok(index) = self
            .terms
            .binary_search_by(|term| term.get_lit().get_var().cmp(&var_idx))
        {
            self.terms.remove(index).get_coeff().clone()
        } else {
            N::zero()
        }
    }

    /// Get the sum of the coefficients for [`GeneralPBConstraint`].
    #[inline]
    fn set_coeff_sum(&mut self, coeff_sum: N) {
        self.coeff_sum = coeff_sum;
    }

    /// Add the `value` the sum of the coefficients in [`GeneralPBConstraint`].
    #[inline]
    fn add_to_coeff_sum(&mut self, value: &N) {
        self.coeff_sum += value;
    }

    /// Set the right-hand side of [`GeneralPBConstraint`] to `degree`.
    #[inline]
    pub fn set_degree(&mut self, degree: N) {
        self.degree = degree;
    }

    /// Create a new pseudo-Boolean constraint from terms and degree. The resulting constraint is normalized.
    ///
    /// To get the smallest possible type, this function should only be used in combination with [`into_smallest_type()`](PBConstraint::into_smallest_type()). This is automatically done by using the functions [`constraint_from_terms()`] or [`constraint_from_terms_and_coeff_sum()`].
    #[inline]
    pub fn from_terms(terms: Vec<GeneralPBTerm<N>>, coeff_sum: N, degree: N) -> Self {
        let mut constraint = GeneralPBConstraint {
            terms,
            coeff_sum,
            degree,
        };
        constraint.normalize();
        constraint
    }

    /// Check if the coefficients of all terms are 1.
    #[inline]
    pub fn all_coeff_one(&self) -> bool {
        for term in self.terms.iter() {
            if term.coeff != One::one() {
                return false;
            }
        }
        true
    }

    /// Add term to pseudo-Boolean constraint. The term is automatically normalized if the term is not normalized.
    #[inline]
    pub fn add_term(&mut self, new_term: GeneralPBTerm<N>) {
        // Check if the variable of the term is already in the constraint.
        for term in self.terms.iter_mut() {
            if term.lit.get_var() == new_term.lit.get_var() {
                term.add_with(new_term);
                return;
            }
        }

        self.terms.push(new_term);
    }

    /// In situ normalizing the constraint.
    #[inline]
    fn normalize(&mut self) {
        if self.terms.is_empty() {
            return;
        }

        // Sort the terms by variable.
        self.terms
            .sort_unstable_by(|a, b| a.lit.get_var().partial_cmp(&b.lit.get_var()).unwrap());

        let mut new = 0;
        for original in 1..self.terms.len() {
            if self.terms[new].lit.get_var() == self.terms[original].lit.get_var() {
                // If both terms have the same variable, just add the literals without and save them at the `new` position.
                let old_term = self.terms[original].clone();
                let cancellation = self.terms[new].add_with(old_term);
                self.degree -= cancellation.clone();
            } else {
                // We are finished with all literals for this variable.
                let finished_term = self.terms.get_mut(new).unwrap();
                // Normalize term, as the term might have negative coefficient.
                if finished_term.coeff.is_negative() {
                    finished_term.change_negation();
                    self.degree += finished_term.coeff.clone();
                }
                // If the coefficient is not 0, i.e., the term exists in the normalized form, then we go to next term.
                if finished_term.coeff != Zero::zero() {
                    new += 1;
                }

                if new != original {
                    self.terms[new] = self.terms[original].clone();
                }
            }
        }

        let last_term = self.terms.get_mut(new).unwrap();
        // Normalize term, as the `last_term` might have negative coefficient.
        if last_term.coeff.is_negative() {
            last_term.change_negation();
            self.degree += last_term.coeff.clone();
        }
        // Final test if the `last_term` has coefficient 0 and can be removed.
        if last_term.coeff.is_zero() {
            self.terms.truncate(new);
        } else {
            self.terms.truncate(new + 1);
        }

        // Recompute the sum of the coefficients.
        self.coeff_sum = Zero::zero();
        for term in self.terms.iter() {
            self.coeff_sum += &term.coeff;
        }
    }

    /// Merge the terms from the constraint given in `summand` into this constraint.
    ///
    /// The returned integer is the cancellation due to merging the terms, i.e., the constant on the left-hand side after normalizing the terms. The cancellation should be subtracted from the degree and subtracted twice from the sum of coefficients.
    #[inline]
    fn merge_terms(&mut self, summand: &impl PBConstraintGetter) -> N {
        let mut cancel = N::zero();
        let mut resulting_terms = Vec::with_capacity(self.terms.len() + summand.len());
        let mut first_term = self.terms.iter();
        let mut second_term = summand.get_terms().iter();
        let mut cur_first = first_term.next();
        let mut cur_second = second_term.next();

        loop {
            match (cur_first, cur_second) {
                (None, None) => break,
                (Some(first), Some(second)) => {
                    match first.lit.get_var().cmp(&second.get_lit().get_var()) {
                        Ordering::Equal => {
                            let second = GeneralPBTerm::new(
                                Into::<BigInt>::into(second.get_coeff().clone())
                                    .try_into()
                                    .ok()
                                    .unwrap(),
                                second.get_lit(),
                            );
                            resulting_terms.push(first.to_owned());
                            cancel += resulting_terms.last_mut().unwrap().add_with(second);
                            if resulting_terms.last().unwrap().coeff.is_zero() {
                                resulting_terms.pop();
                            }

                            cur_first = first_term.next();
                            cur_second = second_term.next();
                        }
                        Ordering::Less => {
                            resulting_terms.push(first.to_owned());
                            cur_first = first_term.next();
                        }
                        Ordering::Greater => {
                            resulting_terms.push(GeneralPBTerm::new(
                                Into::<BigInt>::into(second.get_coeff().clone())
                                    .try_into()
                                    .ok()
                                    .unwrap(),
                                second.get_lit(),
                            ));
                            cur_second = second_term.next();
                        }
                    }
                }
                (None, Some(second)) => {
                    resulting_terms.push(GeneralPBTerm::new(
                        Into::<BigInt>::into(second.get_coeff().clone())
                            .try_into()
                            .ok()
                            .unwrap(),
                        second.get_lit(),
                    ));
                    cur_second = second_term.next();
                }
                (Some(first), None) => {
                    resulting_terms.push(first.to_owned());
                    cur_first = first_term.next();
                }
            }
        }

        self.terms = resulting_terms;

        cancel
    }

    /// Get the coefficient of a literal in the constraint.
    ///
    /// The coefficient is returned as `Some(coefficient)`. If there is no term in the constraint that contains `lit`, then [`None`] is returned.
    #[inline]
    pub fn get_coeff(&self, lit: Lit) -> Option<&N> {
        if let Ok(index) = self.terms.binary_search_by(|t| t.lit.cmp(&lit)) {
            Some(&self.terms[index].coeff)
        } else {
            None
        }
    }
}

impl<N> PBConstraintGetter for GeneralPBConstraint<N>
where
    N: Int,
    PBConstraintEnum: From<GeneralPBConstraint<N>>,
{
    type CoeffType = N;
    type TermType = GeneralPBTerm<N>;

    #[inline]
    fn get_coeff_sum(&self) -> Self::CoeffType {
        self.coeff_sum.clone()
    }

    #[inline]
    fn get_degree(&self) -> &Self::CoeffType {
        &self.degree
    }

    #[inline]
    fn get_terms(&self) -> &Vec<Self::TermType> {
        &self.terms
    }

    #[inline]
    fn get_lits(&self) -> impl Iterator<Item = &Lit> {
        self.get_terms().iter().map(|term| &term.lit)
    }
}

impl<N> PBConstraint for GeneralPBConstraint<N>
where
    N: Int,
    PBConstraintEnum: From<GeneralPBConstraint<N>>,
{
    #[inline]
    fn is_contradicting(&self) -> bool {
        self.degree > self.coeff_sum
    }

    #[inline]
    fn is_trivial(&self) -> bool {
        !self.degree.is_positive()
    }

    #[inline]
    fn len(&self) -> usize {
        self.terms.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    #[inline]
    fn multiply(&mut self, factor: &BigInt) -> Option<PBConstraintEnum> {
        let new_coeff_sum = self.get_coeff_sum() * factor.clone();
        let new_degree = self.get_degree().clone() * factor.clone();

        // Try to do multiplication in situ.
        if let (Ok(coeff_sum), Ok(degree)) = (
            TryInto::<N>::try_into(new_coeff_sum.clone()),
            TryInto::<N>::try_into(new_degree.clone()),
        ) {
            // It should be safe to cast factor to `IntegerType`.
            let factor: N = factor.clone().try_into().ok().unwrap();
            self.degree = degree;
            self.coeff_sum = coeff_sum;
            for term in self.terms.iter_mut() {
                term.set_coeff(term.get_coeff().clone() * factor.clone())
            }
            return None;
        }

        // Multiply into new constraint.
        if let (Ok(coeff_sum), Ok(degree)) = (
            TryInto::<i128>::try_into(&new_coeff_sum),
            TryInto::<i128>::try_into(&new_degree),
        ) {
            let factor: i128 = factor.clone().try_into().ok().unwrap();

            let terms = self
                .terms
                .iter()
                .map(|t| t.multiply_to_i128(factor))
                .collect();

            Some(GeneralPBConstraint::<i128>::from_terms(terms, coeff_sum, degree).into())
        } else {
            let terms = self
                .terms
                .iter()
                .map(|t| t.multiply_to_bigint(factor))
                .collect();

            Some(GeneralPBConstraint::<BigInt>::from_terms(terms, new_coeff_sum, new_degree).into())
        }
    }

    fn add<C: PBConstraintGetter>(&mut self, summand: &C) -> Option<PBConstraintEnum> {
        let coeff_sum = self.get_coeff_sum().into() + summand.get_coeff_sum().into();
        let degree_sum = self.get_degree().clone().into() + summand.get_degree().clone().into();

        // Add second constraint into first constraint in situ.
        if let (Ok(coeff_sum), Ok(degree_sum)) = (
            TryInto::<N>::try_into(coeff_sum.clone()),
            TryInto::<N>::try_into(degree_sum.clone()),
        ) {
            // Mergesort for the terms, adjusting the coeff_sum and degree if there is cancellation.
            let cancellation = self.merge_terms(summand);
            self.set_degree(degree_sum - &cancellation);
            self.set_coeff_sum(coeff_sum - (N::from(2) * cancellation));

            return None;
        }

        // Add second constraint to first constraint which has been cast to `i128`.
        if let (Ok(coeff_sum), Ok(degree_sum)) = (
            TryInto::<i128>::try_into(&coeff_sum),
            TryInto::<i128>::try_into(&degree_sum),
        ) {
            let terms = self
                .terms
                .iter()
                .map(|t| {
                    GeneralPBTerm::<i128>::new(t.coeff.to_owned().try_into().ok().unwrap(), t.lit)
                })
                .collect();
            let mut first_constraint = GeneralPBConstraint::<i128>::from_terms(
                terms,
                self.coeff_sum.to_owned().try_into().ok().unwrap(),
                self.degree.to_owned().try_into().ok().unwrap(),
            );
            let cancellation = first_constraint.merge_terms(summand);
            first_constraint.set_degree(degree_sum - cancellation);
            first_constraint.set_coeff_sum(coeff_sum - (2 * cancellation));

            Some(first_constraint.into())
        } else {
            let terms = self
                .terms
                .iter()
                .map(|t| GeneralPBTerm::<BigInt>::new(t.coeff.to_owned().into(), t.lit))
                .collect();
            let mut first_constraint = GeneralPBConstraint::<BigInt>::from_terms(
                terms,
                self.coeff_sum.to_owned().into(),
                self.degree.to_owned().into(),
            );
            let cancellation = first_constraint.merge_terms(summand);
            first_constraint.set_degree(degree_sum - &cancellation);
            first_constraint.set_coeff_sum(coeff_sum - (2 * cancellation));

            Some(first_constraint.into())
        }
    }

    #[inline]
    fn lower_rhs(&mut self, amount: &BigInt) -> Option<PBConstraintEnum> {
        debug_assert!(!amount.is_negative());

        if let Ok(amount) = TryInto::<N>::try_into(amount.to_owned()) {
            if let Some(degree) = self.degree.checked_sub(&amount) {
                self.degree = degree;
                return None;
            }
        }

        let new_degree = self.degree.to_owned().into() - amount;
        let coeff_sum = self.coeff_sum.to_owned().into();
        let terms = std::mem::take(&mut self.terms)
            .into_iter()
            .map(|t| GeneralPBTerm::new(t.coeff.into(), t.lit))
            .collect();
        let new_constraint =
            GeneralPBConstraint::<BigInt>::from_terms(terms, coeff_sum, new_degree);

        Some(new_constraint.into())
    }

    #[inline]
    fn negate(&self) -> PBConstraintEnum {
        let mut negate_terms = self.terms.clone();
        for term in negate_terms.iter_mut() {
            term.negate();
        }

        GeneralPBConstraint::from_terms(
            negate_terms,
            self.coeff_sum.to_owned(),
            N::one() + &self.coeff_sum - &self.degree,
        )
        .into()
    }

    #[inline]
    fn substitute(&self, substitution: &impl Substitutable) -> PBConstraintEnum {
        let mut substituted_terms = Vec::new();
        let mut substituted_degree = self.degree.clone();
        let mut substituted_coeff_sum = self.coeff_sum.clone();
        for term in self.terms.iter() {
            match substitution.get_lit(term.lit) {
                Some(SubstitutionValue::TRUE) => {
                    substituted_degree -= &term.coeff;
                    substituted_coeff_sum -= &term.coeff;
                }
                Some(SubstitutionValue::FALSE) => substituted_coeff_sum -= &term.coeff,
                Some(substituted_lit) => substituted_terms.push(GeneralPBTerm::new(
                    term.coeff.clone(),
                    substituted_lit.get_lit(),
                )),
                None => substituted_terms.push(term.clone()),
            }
        }

        constraint_from_terms_and_coeff_sum(
            substituted_terms,
            substituted_degree,
            substituted_coeff_sum,
        )
    }

    #[inline]
    fn get_max_coeff(&self) -> BigInt {
        if let Some(min) = self.terms.iter().max_by(|s, t| s.coeff.cmp(&t.coeff)) {
            min.coeff.clone().into()
        } else {
            BigInt::zero()
        }
    }

    #[inline]
    fn get_lit(&self, index: usize) -> Option<&Lit> {
        self.terms.get(index).map(|term| &term.lit)
    }

    #[inline]
    fn get_term(&self, index: usize) -> Option<GeneralPBTerm<BigInt>> {
        self.terms
            .get(index)
            .map(|term| GeneralPBTerm::new(term.coeff.clone().into(), term.lit))
    }

    #[inline]
    fn is_satisfied(&self, assignment: &Assignment<BooleanVar>) -> bool {
        if self.is_trivial() {
            return true;
        }

        let mut counter = N::zero();
        for term in self.terms.iter() {
            if unsafe { assignment.get_lit_value_unchecked(term.lit) } == BoolValue::Assigned(true)
            {
                counter += &term.coeff;
                if counter >= self.degree {
                    return true;
                }
            }
        }

        false
    }

    #[inline]
    fn is_falsified(&self, assignment: &Assignment<BooleanVar>) -> bool {
        if self.is_contradicting() {
            return true;
        }

        let mut counter = self.coeff_sum.clone();
        for term in self.terms.iter() {
            if assignment.get_lit_value(term.lit) == BoolValue::Assigned(false) {
                counter -= &term.coeff;
                if counter < self.degree {
                    return true;
                }
            }
        }

        false
    }

    #[inline]
    fn saturate(&mut self) {
        if self.is_trivial() {
            self.terms.clear();
            self.set_coeff_sum(N::zero());
            return;
        }

        let mut coeff_sum_change = N::zero();
        for term in self.terms.iter_mut() {
            if term.get_coeff() > &self.degree {
                coeff_sum_change -= term.get_coeff().abs_sub(&self.degree);
                term.set_coeff(self.degree.clone());
            }
        }
        self.add_to_coeff_sum(&coeff_sum_change);
    }

    #[inline]
    fn weaken(&mut self, var_idx: VarIdx) -> Option<PBConstraintEnum> {
        let coeff = self.delete_term(var_idx);
        self.degree -= &coeff;

        // Subtract coefficient from coefficient sum.
        self.add_to_coeff_sum(&-coeff);
        None
    }

    #[inline]
    fn weaken_all(&mut self, mut var_idxs: Vec<VarIdx>) -> Option<PBConstraintEnum> {
        if var_idxs.len() <= WEAKEN_ALL_THRESHOLD {
            // Weaken all variables in var_idxs by calling the remove function.
            let mut weaken_sum = N::zero();
            for var_idx in var_idxs.into_iter() {
                weaken_sum += self.delete_term(var_idx);
            }
            self.degree -= &weaken_sum;
            self.add_to_coeff_sum(&-weaken_sum);
        } else {
            // Weaken all variables by calling the remove function.
            var_idxs.sort_unstable();
            let mut weaken_vars = var_idxs.iter();
            let mut cur_var = weaken_vars.next();
            let mut weaken_sum = N::zero();
            let mut index = 0;
            let mut new_index = 0;

            while let Some(var) = cur_var {
                if index == self.len() {
                    break;
                }
                match self.terms[index].lit.get_var().cmp(var) {
                    Ordering::Equal => {
                        // The current variable is weakened away.
                        weaken_sum += self.terms[index].get_coeff();
                        cur_var = weaken_vars.next();
                        index += 1;
                    }
                    Ordering::Less => {
                        // The current variable is not weakened, so move it to keep `terms` contiguous.
                        self.terms[new_index] = self.terms[index].clone();
                        new_index += 1;
                        index += 1;
                    }
                    Ordering::Greater => {
                        // The weakened variable is not in the constraint.
                        cur_var = weaken_vars.next();
                    }
                }
            }
            while index < self.len() {
                // Move remaining variables to keep `terms` contiguous.
                self.terms[new_index] = self.terms[index].clone();
                new_index += 1;
                index += 1;
            }
            self.terms.truncate(new_index);
            self.degree -= &weaken_sum;
            self.add_to_coeff_sum(&-weaken_sum);
        }
        None
    }

    #[inline]
    fn normalized_form_div(&mut self, divisor: &BigInt) {
        match divisor.clone().try_into() {
            Ok(divisor) => {
                let mut new_coeff_sum = N::zero();
                for term in self.terms.iter_mut() {
                    term.divide_round_up(&divisor);
                    new_coeff_sum += term.get_coeff();
                }
                self.set_coeff_sum(new_coeff_sum);
                self.set_degree(self.get_degree().div_ceil(&divisor));
            }
            Err(_) => {
                for term in self.terms.iter_mut() {
                    term.set_coeff(N::one());
                }
                self.set_coeff_sum((self.len() as i64).into());
                if !self.get_degree().is_positive() {
                    self.set_degree(N::zero());
                } else {
                    self.set_degree(N::one());
                }
            }
        };
    }

    #[inline]
    fn variable_form_div(&mut self, divisor: &BigInt) -> Option<PBConstraintEnum> {
        let mut new_neg_coeff_sum = N::zero();
        match divisor.clone().try_into() {
            Ok(divisor) => {
                let mut new_coeff_sum = N::zero();
                self.terms.retain_mut(|term| {
                    if term.lit.is_negated() {
                        self.degree -= &term.coeff;
                        term.coeff /= &divisor;
                        new_neg_coeff_sum += &term.coeff;
                    } else {
                        term.divide_round_up(&divisor);
                    }
                    new_coeff_sum += &term.coeff;
                    !term.coeff.is_zero()
                });
                self.set_coeff_sum(new_coeff_sum);
                self.degree =
                    num_integer::Integer::div_ceil(&self.degree, &divisor) + new_neg_coeff_sum;
            }
            Err(_) => {
                self.terms.retain_mut(|term| {
                    if term.lit.is_negated() {
                        self.degree -= &term.coeff;
                        false
                    } else {
                        term.coeff = N::one();
                        true
                    }
                });
                self.set_coeff_sum((self.terms.len() as i64).into());
                if !self.degree.is_positive() {
                    self.degree = N::zero();
                } else {
                    self.degree = N::one();
                }
            }
        };

        None
    }

    #[inline]
    fn normalized_form_mir(&mut self, divisor: &BigInt) -> Option<PBConstraintEnum> {
        // Compute the remainder of (A mod d).
        let remainder = self.degree.clone().into().mod_floor(divisor);
        // Special case if remainder is zero.
        if remainder.is_zero() {
            self.terms.clear();
            self.coeff_sum = N::zero();
            self.degree = N::zero();
            return None;
        }

        let mut new_coeff_sum = N::zero();
        // Try if we can perform the MIR cut in situ.
        if let Ok(divisor) = TryInto::<N>::try_into(divisor.clone()) {
            let remainder = remainder.try_into().unwrap_or(N::one());
            for term in self.terms.iter_mut() {
                term.coeff = (term.coeff.div_floor(&divisor) * &remainder)
                    + (&term.coeff.mod_floor(&divisor)).min(&remainder);
                new_coeff_sum += &term.coeff;
            }
            self.degree = self.degree.div_ceil(&divisor) * remainder;
        } else {
            // Divisor is larger than coeff sum, then all coefficients are min of remainders.
            if let Ok(remainder) = TryInto::<N>::try_into(remainder.clone()) {
                for term in self.terms.iter_mut() {
                    if term.coeff > remainder {
                        term.coeff = remainder.clone();
                    }
                    new_coeff_sum += &term.coeff;
                }
                // Degree becomes remainder.
                self.degree = remainder;
            } else {
                // Remainder is larger than coeff sum, hence A <= 0.
                self.degree = N::zero();
                new_coeff_sum = self.coeff_sum.clone();
            }
        }
        self.coeff_sum = new_coeff_sum;

        None
    }

    #[inline]
    fn variable_form_mir(&mut self, divisor: &BigInt) -> Option<PBConstraintEnum> {
        self.degree -= self
            .terms
            .iter()
            .filter(|t| t.lit.is_negated())
            .fold(N::zero(), |sum, t| sum + &t.coeff);

        // Compute the remainder of (A mod d).
        let remainder = self.degree.clone().into().mod_floor(divisor);
        // Special case if remainder is zero.
        if remainder.is_zero() {
            self.terms.clear();
            self.coeff_sum = N::zero();
            self.degree = N::zero();
            return None;
        }

        let mut new_coeff_sum = N::zero();
        let mut new_neg_coeff_sum = N::zero();
        // Try if we can perform the MIR cut in situ.
        if let Ok(divisor) = TryInto::<N>::try_into(divisor.clone()) {
            let remainder = remainder
                .try_into()
                .unwrap_or_else(|_| panic!("remainder should be small enough"));
            self.terms.retain_mut(|term| {
                if term.lit.is_negated() {
                    term.coeff = (term.coeff.div_ceil(&divisor) * &remainder)
                        - (&(-term.coeff.clone()).mod_floor(&divisor)).min(&remainder);
                    new_neg_coeff_sum += &term.coeff;
                } else {
                    term.coeff = (term.coeff.div_floor(&divisor) * &remainder)
                        + (&term.coeff.mod_floor(&divisor)).min(&remainder);
                }
                new_coeff_sum += &term.coeff;
                !term.coeff.is_zero()
            });
            self.degree = self.degree.div_ceil(&divisor) * remainder + new_neg_coeff_sum;
        } else {
            // Divisor is larger than coeff sum, then positive coefficients are min of remainders and negative coefficients are 1 - remainder.
            if let Ok(remainder_small_type) = TryInto::<N>::try_into(remainder.clone()) {
                // Remainder can still be represented correctly.
                self.terms.retain_mut(|term| {
                    if term.lit.is_negated() {
                        // Compute remainder of coefficient correctly in `BigInt`.
                        let coeff_remainder = -term.coeff.clone().into() + divisor;
                        if coeff_remainder >= remainder {
                            return false;
                        } else {
                            // The resulting (a_i mod d) - (A mod d) is negative, hence (A mod d) - (a_i mod d) is positive.
                            term.coeff = (-coeff_remainder + &remainder)
                                .try_into()
                                .unwrap_or_else(|_| panic!("sum should be small enough"))
                        }
                        new_neg_coeff_sum += &term.coeff;
                    } else if term.coeff > remainder_small_type {
                        term.coeff = remainder_small_type.clone();
                    }
                    new_coeff_sum += &term.coeff;
                    true
                });
                // Degree becomes remainder.
                self.degree = remainder_small_type + new_neg_coeff_sum;
            } else {
                // Remainder is larger than coeff sum, hence A <= 0.
                let neg_degree = -self.degree.clone();
                self.terms.retain_mut(|term| {
                    if term.lit.is_negated() {
                        // If term is negated and a_i < A, then coeff is a_i - A, but a_i is stored negated, so -a_i < -A and then -a_i - -A.
                        if term.coeff > neg_degree {
                            term.coeff -= &neg_degree;
                        } else {
                            return false;
                        }
                        new_neg_coeff_sum += &term.coeff;
                    }
                    new_coeff_sum += &term.coeff;
                    true
                });
                self.degree = N::zero() + new_neg_coeff_sum;
            }
        }
        self.coeff_sum = new_coeff_sum;

        None
    }

    #[inline]
    fn propagate(&self, assignment: &mut Assignment<BooleanVar>) -> ConstraintPropagationResult {
        if self.is_trivial() {
            return ConstraintPropagationResult::NoPropagation;
        }

        let mut slack = -self.degree.clone();
        for term in self.terms.iter() {
            if unsafe { assignment.get_lit_value_unchecked(term.lit) } != BoolValue::Assigned(false)
            {
                slack += term.coeff.to_owned();
            }
        }

        if slack.is_negative() {
            return ConstraintPropagationResult::Conflict;
        }

        let mut propagated = false;
        for term in self.terms.iter() {
            if unsafe { assignment.get_lit_value_unchecked(term.lit) } == BoolValue::Unassigned
                && term.coeff > slack
            {
                propagated = true;
                unsafe { assignment.set_lit_value_unchecked(term.lit, BoolValue::Assigned(true)) };
            }
        }

        if propagated {
            ConstraintPropagationResult::Propagated
        } else {
            ConstraintPropagationResult::NoPropagation
        }
    }

    #[inline]
    fn traced_propagate(&self, assignment: &mut Assignment<BooleanVar>) -> Vec<Lit> {
        if self.is_trivial() {
            return vec![];
        }

        let mut slack = -self.degree.clone();
        for term in self.terms.iter() {
            if unsafe { assignment.get_lit_value_unchecked(term.lit) } != BoolValue::Assigned(false)
            {
                slack += term.coeff.to_owned();
            }
        }

        if slack.is_negative() {
            panic!("This did not propagate to conflict earlier!")
        }

        let mut lits = Vec::new();
        for term in self.terms.iter() {
            if unsafe { assignment.get_lit_value_unchecked(term.lit) } == BoolValue::Unassigned
                && term.coeff > slack
            {
                lits.push(term.lit);
                unsafe { assignment.set_lit_value_unchecked(term.lit, BoolValue::Assigned(true)) };
            }
        }

        lits
    }

    #[inline]
    fn mark_negated_lits(
        &self,
        assignment: &Assignment<BooleanVar>,
        marking: &mut Assignment<BooleanVar>,
    ) {
        for &lit in self.get_lits() {
            if unsafe { assignment.get_lit_value_unchecked(lit) } == BoolValue::Assigned(false) {
                unsafe { marking.set_lit_value_unchecked(lit, BoolValue::Assigned(false)) };
            }
        }
    }

    fn into_smallest_type(self) -> PBConstraintEnum {
        // Check if we can use Clause or cardinality constraint.
        if self.all_coeff_one() {
            if self.degree.is_one() {
                return Clause::from_lits(self.terms.iter().map(|t| t.lit).collect()).into();
            } else if let Ok(degree) = self.degree.to_owned().try_into() {
                return Cardinality::from_lits(self.terms.iter().map(|t| t.lit).collect(), degree)
                    .into();
            };
        }

        // General PB constraint. Check for smallest possible type.
        if let (Ok(coeff_sum), Ok(degree)) = (
            TryInto::<i64>::try_into(self.coeff_sum.clone()),
            TryInto::<i64>::try_into(self.degree.clone()),
        ) {
            if TypeId::of::<N>() == TypeId::of::<i64>() {
                return self.into();
            }
            GeneralPBConstraint::from_terms(
                self.terms
                    .into_iter()
                    .map(|t| {
                        GeneralPBTerm::new(
                            unsafe { TryInto::<i64>::try_into(t.coeff).unwrap_unchecked() },
                            t.lit,
                        )
                    })
                    .collect(),
                coeff_sum,
                degree,
            )
            .into()
        } else if let (Ok(coeff_sum), Ok(degree)) = (
            TryInto::<i128>::try_into(self.coeff_sum.clone()),
            TryInto::<i128>::try_into(self.degree.clone()),
        ) {
            if TypeId::of::<N>() == TypeId::of::<i128>() {
                return self.into();
            }
            GeneralPBConstraint::from_terms(
                self.terms
                    .into_iter()
                    .map(|t| {
                        GeneralPBTerm::new(
                            unsafe { TryInto::<i128>::try_into(t.coeff).unwrap_unchecked() },
                            t.lit,
                        )
                    })
                    .collect(),
                coeff_sum,
                degree,
            )
            .into()
        } else {
            self.into()
        }
    }
}

impl<N: Int> From<&Clause> for GeneralPBConstraint<N> {
    #[inline]
    fn from(value: &Clause) -> Self {
        let terms: Vec<_> = value
            .get_lits()
            .map(|&l| GeneralPBTerm::new(N::one(), l))
            .collect();
        GeneralPBConstraint {
            terms,
            coeff_sum: N::from(value.len() as i64),
            degree: N::one(),
        }
    }
}

impl<N: Int> From<&Cardinality> for GeneralPBConstraint<N> {
    #[inline]
    fn from(value: &Cardinality) -> Self {
        let terms: Vec<_> = value
            .get_lits()
            .map(|&l| GeneralPBTerm::new(N::one(), l))
            .collect();
        GeneralPBConstraint {
            terms,
            coeff_sum: N::from(value.len() as i64),
            degree: N::from(*value.get_degree()),
        }
    }
}

impl<N: Int> ToPrettyString for GeneralPBConstraint<N> {
    #[inline]
    fn to_pretty_string(&self, var_names: &VarNameManager) -> String {
        let mut out = String::with_capacity(self.terms.len() * 4);
        for term in self.terms.iter() {
            out.push_str(&term.coeff.to_string());
            out.push(' ');
            out.push_str(&term.lit.to_pretty_string(var_names));
            out.push(' ');
        }
        out.push_str(">= ");
        out.push_str(&self.degree.to_string());
        out
    }
}
