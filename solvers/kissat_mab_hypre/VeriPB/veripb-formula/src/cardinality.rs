//! Implementation of [`PBConstraint`] for cardinality constraints.

use std::cmp::Ordering;

use malachite_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, Zero};

use crate::{
    clause::Clause,
    helper::{merge_from_lits, merge_in_situ_lits},
    lit::Lit,
    pb_constraint::{
        constraint_from_terms_and_coeff_sum, PBConstraint, PBConstraintEnum, PBConstraintGetter,
        WEAKEN_ALL_THRESHOLD,
    },
    prelude::*,
    substitution::Substitutable,
    to_pretty_string::ToPrettyString,
    var_name_manager::VarNameManager,
};

/// A cardinality is constraint has all cefficients 1 and the right-hand side can be any integer.
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub struct Cardinality {
    lits: Vec<Lit>,
    degree: i64,
}

impl Cardinality {
    /// Set the right-hand side of [`Cardinality`] to `degree`.
    #[inline]
    pub fn set_degree(&mut self, degree: i64) {
        self.degree = degree;
    }

    /// Delete the term containing the [`VarIdx`] `var_idx` from the [`Cardinality`] constraint.
    #[inline]
    fn delete_term(&mut self, var_idx: VarIdx) -> i64 {
        if let Ok(index) = self
            .lits
            .binary_search_by(|lit| lit.get_var().cmp(&var_idx))
        {
            self.lits.remove(index);
            1
        } else {
            0
        }
    }

    /// Get the sum of the coefficients for [`Cardinality`].
    #[inline]
    fn get_coeff_sum(&self) -> i64 {
        self.len() as i64
    }

    /// Get the literals stored in [`Cardinality`] as a slice of [`Lit`].
    #[inline]
    pub fn as_slice(&self) -> &[Lit] {
        self.lits.as_slice()
    }

    /// Create a [`Cardinality`] from [`Vec<Lit>`] terms and the right-hand side `degree`.
    #[inline]
    pub fn from_lits(lits: Vec<Lit>, degree: i64) -> Self {
        Cardinality { lits, degree }
    }
}

impl PBConstraintGetter for Cardinality {
    type CoeffType = i64;
    type TermType = Lit;

    #[inline]
    fn get_coeff_sum(&self) -> Self::CoeffType {
        self.lits.len() as i64
    }

    #[inline]
    fn get_degree(&self) -> &Self::CoeffType {
        &self.degree
    }

    #[inline]
    fn get_terms(&self) -> &Vec<Self::TermType> {
        &self.lits
    }

    #[inline]
    fn get_lits(&self) -> impl Iterator<Item = &Lit> {
        self.lits.iter()
    }
}

impl PBConstraint for Cardinality {
    #[inline]
    fn is_contradicting(&self) -> bool {
        self.degree > self.lits.len() as i64
    }

    #[inline]
    fn is_trivial(&self) -> bool {
        !self.degree.is_positive()
    }

    #[inline]
    fn saturate(&mut self) {
        if self.is_trivial() {
            self.lits.clear();
        }
    }

    #[inline]
    fn weaken(&mut self, var_idx: VarIdx) -> Option<PBConstraintEnum> {
        self.degree -= self.delete_term(var_idx);
        None
    }

    #[inline]
    fn weaken_all(&mut self, mut var_idxs: Vec<VarIdx>) -> Option<PBConstraintEnum> {
        if var_idxs.len() <= WEAKEN_ALL_THRESHOLD {
            // Weaken all variables in var_idxs by calling the remove function
            for var_idx in var_idxs.into_iter() {
                self.degree -= self.delete_term(var_idx);
            }
        } else {
            // Weaken all variables by calling the remove function.
            var_idxs.sort_unstable();
            let mut weaken_vars = var_idxs.iter();
            let mut cur_var = weaken_vars.next();
            let mut index = 0;
            let mut new_index = 0;

            while let Some(var) = cur_var {
                if index == self.len() {
                    break;
                }
                match self.lits[index].get_var().cmp(var) {
                    Ordering::Equal => {
                        // The current variable is weakened away.
                        self.degree -= 1;
                        cur_var = weaken_vars.next();
                        index += 1;
                    }
                    Ordering::Less => {
                        // The current variable is not weakened, so move it to keep `lits` contiguous.
                        self.lits[new_index] = self.lits[index];
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
                // Move remaining variables to keep `lits` contiguous.
                self.lits[new_index] = self.lits[index];
                new_index += 1;
                index += 1;
            }
            self.lits.truncate(new_index);
        }
        None
    }

    #[inline]
    fn normalized_form_div(&mut self, divisor: &BigInt) {
        let divisor = divisor.try_into().unwrap_or(i64::MAX);
        self.degree = num_integer::Integer::div_ceil(&self.degree, &divisor);
    }

    #[inline]
    fn variable_form_div(&mut self, divisor: &BigInt) -> Option<PBConstraintEnum> {
        if divisor.is_one() {
            return None;
        }

        // Positive literals are retained.
        self.lits.retain(|lit| {
            if lit.is_negated() {
                self.degree -= 1;
            }
            !lit.is_negated()
        });

        let divisor = divisor.try_into().unwrap_or(i64::MAX);
        self.degree = num_integer::Integer::div_ceil(&self.degree, &divisor);

        None
    }

    #[inline]
    fn normalized_form_mir(&mut self, divisor: &BigInt) -> Option<PBConstraintEnum> {
        // Compute the remainder of (A mod d).
        let remainder = BigInt::from(self.degree).mod_floor(divisor);
        // Special case is remainder is 0.
        if remainder.is_zero() {
            self.lits.clear();
            self.degree = 0;
            return None;
        }

        // Compute the quotient.
        let divisor = divisor.try_into().unwrap_or(i64::MAX);
        self.degree = num_integer::Integer::div_ceil(&self.degree, &divisor);

        if let Ok(remainder) = TryInto::<i64>::try_into(&remainder) {
            // Compute the new degree.
            self.degree *= remainder;
        }

        None
    }

    #[inline]
    fn variable_form_mir(&mut self, divisor: &BigInt) -> Option<PBConstraintEnum> {
        // Positive literals are retained.
        self.lits.retain(|lit| {
            if lit.is_negated() {
                self.degree -= 1;
            }
            !lit.is_negated()
        });

        // Compute the remainder of (A mod d).
        let remainder = BigInt::from(self.degree).mod_floor(divisor);
        // Special case is remainder is 0.
        if remainder.is_zero() {
            self.lits.clear();
            self.degree = 0;
            return None;
        }

        // Compute the quotient.
        let divisor = divisor.try_into().unwrap_or(i64::MAX);
        self.degree = num_integer::Integer::div_ceil(&self.degree, &divisor);

        if let Ok(remainder) = TryInto::<i64>::try_into(&remainder) {
            // Compute the new degree.
            self.degree *= remainder;
        }

        None
    }

    #[inline]
    fn multiply(&mut self, factor: &BigInt) -> Option<PBConstraintEnum> {
        if factor.is_one() {
            return None;
        }

        let coeff_sum = factor * self.len();
        let degree = factor * self.degree;

        // Try i64 type.
        if let (Ok(coeff_sum), Ok(factor), Ok(degree)) = (
            TryInto::<i64>::try_into(&coeff_sum),
            TryInto::<i64>::try_into(factor),
            TryInto::<i64>::try_into(&degree),
        ) {
            let terms = self
                .lits
                .iter()
                .map(|lit| GeneralPBTerm::new(factor, *lit))
                .collect();
            return Some(GeneralPBConstraint::from_terms(terms, coeff_sum, degree).into());
        }

        if let (Ok(coeff_sum), Ok(factor), Ok(degree)) = (
            TryInto::<i128>::try_into(&coeff_sum),
            TryInto::<i128>::try_into(factor),
            TryInto::<i128>::try_into(&degree),
        ) {
            let terms = self
                .lits
                .iter()
                .map(|lit| GeneralPBTerm::new(factor, *lit))
                .collect();
            return Some(GeneralPBConstraint::from_terms(terms, coeff_sum, degree).into());
        }

        let terms = self
            .lits
            .iter()
            .map(|lit| GeneralPBTerm::new(factor.clone(), *lit))
            .collect();
        Some(GeneralPBConstraint::from_terms(terms, coeff_sum, degree).into())
    }

    fn add<C: PBConstraintGetter>(&mut self, summand: &C) -> Option<PBConstraintEnum> {
        let mut card_term = self.lits.iter().peekable();
        let mut summand_term = summand.get_terms().iter().peekable();
        let mut cancel = 0;
        let mut resulting_lits = Vec::with_capacity(self.len() + summand.len());

        // Try to keep `Cardinality` type first.
        merge_in_situ_lits(
            &mut card_term,
            &mut summand_term,
            &mut resulting_lits,
            &mut cancel,
        );

        // If all terms where merged then it is either a clause or cardinality constraint.
        if card_term.peek().is_none() && summand_term.peek().is_none() {
            if let Ok(degree) = TryInto::<i64>::try_into(summand.get_degree().to_owned()) {
                if let Some(degree) = degree
                    .checked_add(self.degree)
                    .and_then(|d| d.checked_sub(cancel))
                {
                    if degree == 1 {
                        return Some(Clause::from_lits(resulting_lits).into());
                    } else {
                        self.lits = resulting_lits;
                        self.degree = degree;
                        return None;
                    }
                }
            }
        }

        // Change cardinality to general PB constraint.
        let coeff_sum = self.get_coeff_sum() + summand.get_coeff_sum().into();
        let degree_sum = Into::<BigInt>::into(*self.get_degree())
            + Into::<BigInt>::into(summand.get_degree().to_owned());
        if let (Ok(coeff_sum), Ok(degree_sum)) = (
            TryInto::<i64>::try_into(&coeff_sum),
            TryInto::<i64>::try_into(&degree_sum),
        ) {
            let mut resulting_terms: Vec<GeneralPBTerm<i64>> = resulting_lits
                .into_iter()
                .map(|lit| GeneralPBTerm::new(1, lit))
                .collect();

            merge_from_lits(card_term, summand_term, &mut resulting_terms, &mut cancel);
            let constraint = GeneralPBConstraint::from_terms(
                resulting_terms,
                coeff_sum - (2 * cancel),
                degree_sum - cancel,
            );

            Some(constraint.into())
        } else if let (Ok(coeff_sum), Ok(degree_sum)) = (
            TryInto::<i128>::try_into(&coeff_sum),
            TryInto::<i128>::try_into(&degree_sum),
        ) {
            let mut resulting_terms: Vec<GeneralPBTerm<i128>> = resulting_lits
                .into_iter()
                .map(|lit| GeneralPBTerm::new(1, lit))
                .collect();
            let mut cancel = cancel.into();

            merge_from_lits(card_term, summand_term, &mut resulting_terms, &mut cancel);
            let constraint = GeneralPBConstraint::from_terms(
                resulting_terms,
                coeff_sum - (2 * cancel),
                degree_sum - cancel,
            );

            Some(constraint.into())
        } else {
            let mut resulting_terms: Vec<GeneralPBTerm<BigInt>> = resulting_lits
                .into_iter()
                .map(|lit| GeneralPBTerm::new(BigInt::one(), lit))
                .collect();
            let mut cancel = cancel.into();

            merge_from_lits(card_term, summand_term, &mut resulting_terms, &mut cancel);
            let constraint = GeneralPBConstraint::from_terms(
                resulting_terms,
                coeff_sum - (2 * &cancel),
                degree_sum - cancel,
            );

            Some(constraint.into())
        }
    }

    #[inline]
    fn lower_rhs(&mut self, amount: &BigInt) -> Option<PBConstraintEnum> {
        debug_assert!(!amount.is_negative());

        if let Ok(amount) = TryInto::<i64>::try_into(amount) {
            if let Some(degree) = self.degree.checked_sub(amount) {
                self.degree = degree;
                return None;
            }
        }

        GeneralPBConstraint::<BigInt>::from(&*self).lower_rhs(amount)
    }

    #[inline]
    fn negate(&self) -> PBConstraintEnum {
        let mut negated_lits = self.lits.clone();
        for lit in negated_lits.iter_mut() {
            lit.negate();
        }

        Cardinality::from_lits(negated_lits, 1 + self.get_coeff_sum() - self.degree).into()
    }

    #[inline]
    fn substitute(&self, substitution: &impl Substitutable) -> PBConstraintEnum {
        let mut substituted_lits = Vec::new();
        let mut substituted_degree = self.degree;
        for &lit in self.lits.iter() {
            match substitution.get_lit(lit) {
                Some(SubstitutionValue::TRUE) => substituted_degree -= 1,
                Some(SubstitutionValue::FALSE) => {}
                Some(substituted_lit) => {
                    substituted_lits.push(GeneralPBTerm::new(1i64, substituted_lit.get_lit()))
                }
                None => substituted_lits.push(GeneralPBTerm::new(1i64, lit)),
            }
        }

        let coeff_sum = substituted_lits.len() as i64;
        constraint_from_terms_and_coeff_sum(substituted_lits, substituted_degree, coeff_sum)
    }

    #[inline]
    fn get_lit(&self, index: usize) -> Option<&Lit> {
        self.lits.get(index)
    }

    #[inline]
    fn get_term(&self, index: usize) -> Option<GeneralPBTerm<BigInt>> {
        self.lits
            .get(index)
            .map(|lit| GeneralPBTerm::new(1.into(), *lit))
    }

    #[inline]
    fn is_satisfied(&self, assignment: &Assignment<BooleanVar>) -> bool {
        if self.is_trivial() {
            return true;
        }

        let mut counter = 0;
        for &lit in self.lits.iter() {
            if unsafe { assignment.get_lit_value_unchecked(lit) } == BoolValue::Assigned(true) {
                counter += 1;
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

        let mut counter = self.lits.len() as i64;
        for &lit in self.lits.iter() {
            if assignment.get_lit_value(lit) == BoolValue::Assigned(false) {
                counter -= 1;
                if counter < self.degree {
                    return true;
                }
            }
        }

        false
    }

    #[inline]
    fn len(&self) -> usize {
        self.lits.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.lits.is_empty()
    }

    #[inline]
    fn propagate(&self, assignment: &mut Assignment<BooleanVar>) -> ConstraintPropagationResult {
        if self.is_trivial() {
            return ConstraintPropagationResult::NoPropagation;
        }

        let mut unassigned_lits = Vec::new();
        let mut slack = -self.degree;
        for &lit in self.lits.iter() {
            match unsafe { assignment.get_lit_value_unchecked(lit) } {
                BoolValue::Assigned(true) => {
                    slack += 1;
                    if slack > 0 {
                        return ConstraintPropagationResult::NoPropagation;
                    }
                }
                BoolValue::Assigned(false) => {}
                BoolValue::Unassigned => {
                    slack += 1;
                    if slack > 0 {
                        return ConstraintPropagationResult::NoPropagation;
                    }
                    unassigned_lits.push(lit);
                }
            }
        }

        if slack.is_negative() {
            ConstraintPropagationResult::Conflict
        } else if unassigned_lits.is_empty() {
            ConstraintPropagationResult::NoPropagation
        } else {
            for lit in unassigned_lits {
                unsafe { assignment.set_lit_value_unchecked(lit, BoolValue::Assigned(true)) };
            }
            ConstraintPropagationResult::Propagated
        }
    }

    #[inline]
    fn traced_propagate(&self, assignment: &mut Assignment<BooleanVar>) -> Vec<Lit> {
        if self.is_trivial() {
            return vec![];
        }

        let mut unassigned_lits = Vec::new();
        let mut slack = -self.degree;
        for &lit in self.lits.iter() {
            match unsafe { assignment.get_lit_value_unchecked(lit) } {
                BoolValue::Assigned(true) => {
                    slack += 1;
                    if slack > 0 {
                        return vec![];
                    }
                }
                BoolValue::Assigned(false) => {}
                BoolValue::Unassigned => {
                    slack += 1;
                    if slack > 0 {
                        return vec![];
                    }
                    unassigned_lits.push(lit);
                }
            }
        }

        if slack.is_negative() {
            panic!("The propagation did not succeed earlier.")
        } else if unassigned_lits.is_empty() {
            vec![]
        } else {
            let mut lits = Vec::new();
            for lit in unassigned_lits {
                unsafe { assignment.set_lit_value_unchecked(lit, BoolValue::Assigned(true)) };
                lits.push(lit);
            }
            lits
        }
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

    #[inline]
    fn into_smallest_type(self) -> PBConstraintEnum {
        if self.degree.is_one() {
            Clause::from_lits(self.lits).into()
        } else {
            self.into()
        }
    }
}

impl From<&Clause> for Cardinality {
    fn from(value: &Clause) -> Self {
        Cardinality::from_lits(value.get_lits().copied().collect(), 1)
    }
}

impl ToPrettyString for Cardinality {
    #[inline]
    fn to_pretty_string(&self, var_names: &VarNameManager) -> String {
        let mut out = String::with_capacity(self.len() * 4);
        for lit in self.lits.iter() {
            out.push_str("1 ");
            out.push_str(&lit.to_pretty_string(var_names));
            out.push(' ');
        }
        out.push_str(">= ");
        out.push_str(&self.degree.to_string());
        out
    }
}
