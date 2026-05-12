//! Implementation of [`PBConstraint`] for clauses.

use std::cmp::Ordering;

use malachite_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, Zero};

use crate::{
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

/// A [`Clause`] is a pseudo-Boolean constraint where all coefficients are 1 and the right-hand side is 1.
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub struct Clause {
    lits: Vec<Lit>,
}

impl Clause {
    /// Delete the term containing the [`VarIdx`] `var_idx` from the [`Clause`].
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

    /// Get the literals stored in [`Clause`] as a slice of [`Lit`].
    #[inline]
    pub fn as_slice(&self) -> &[Lit] {
        self.lits.as_slice()
    }

    /// Create a [`Clause`] from [`Vec<Lit>`] terms.
    #[inline]
    pub fn from_lits(lits: Vec<Lit>) -> Self {
        Clause { lits }
    }

    /// Create a [`Clause`] from unnormalized [`Vec<Lit>`].
    ///
    /// This function applies a normalization to the `lits`, which removes any duplicate literals. This is the same way as clauses are interpreted in the DIMACS CNF format.
    #[inline]
    pub fn from_unnormalized_lits(mut lits: Vec<Lit>) -> Self {
        // Sort literals.
        lits.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

        // Eliminate duplicate literals.
        let mut new_idx = 0;

        for original_idx in 1..lits.len() {
            if lits[original_idx] != lits[new_idx] {
                new_idx += 1;

                lits[new_idx] = lits[original_idx];
            }
        }

        lits.truncate(new_idx + 1);

        lits.shrink_to_fit();

        Clause { lits }
    }
}

impl PBConstraintGetter for Clause {
    type CoeffType = i64;
    type TermType = Lit;

    #[inline]
    fn get_coeff_sum(&self) -> Self::CoeffType {
        self.lits.len() as i64
    }

    #[inline]
    fn get_degree(&self) -> &Self::CoeffType {
        &1_i64
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

impl PBConstraint for Clause {
    #[inline]
    fn is_contradicting(&self) -> bool {
        self.lits.is_empty()
    }

    #[inline]
    fn is_trivial(&self) -> bool {
        false
    }

    #[inline]
    fn saturate(&mut self) {}

    #[inline]
    fn normalized_form_div(&mut self, _divisor: &BigInt) {}

    #[inline]
    fn variable_form_div(&mut self, divisor: &BigInt) -> Option<PBConstraintEnum> {
        if divisor.is_one() {
            return None;
        }

        let mut variable_form_rhs: i64 = 1;

        // Positive literals are retained.
        self.lits.retain(|lit| {
            if lit.is_negated() {
                variable_form_rhs -= 1;
            }
            !lit.is_negated()
        });

        let divisor = divisor.try_into().unwrap_or(i64::MAX);
        variable_form_rhs = num_integer::Integer::div_ceil(&variable_form_rhs, &divisor);

        if variable_form_rhs == 1 {
            None
        } else {
            Some(Cardinality::from_lits(std::mem::take(&mut self.lits), variable_form_rhs).into())
        }
    }

    #[inline]
    fn normalized_form_mir(&mut self, divisor: &BigInt) -> Option<PBConstraintEnum> {
        // Special case if we divide by 1.
        if divisor.is_one() {
            return Some(Cardinality::from_lits(vec![], 0).into());
        }

        None
    }

    #[inline]
    fn variable_form_mir(&mut self, divisor: &BigInt) -> Option<PBConstraintEnum> {
        let mut variable_form_rhs: i64 = 1;

        // Positive literals are retained.
        self.lits.retain(|lit| {
            if lit.is_negated() {
                variable_form_rhs -= 1;
            }
            !lit.is_negated()
        });

        let remainder = BigInt::from(variable_form_rhs).mod_floor(divisor);
        // Special case if remainder is zero.
        if remainder.is_zero() {
            return Some(Cardinality::from_lits(vec![], 0).into());
        }
        let divisor = divisor.try_into().unwrap_or(i64::MAX);
        variable_form_rhs = num_integer::Integer::div_ceil(&variable_form_rhs, &divisor);
        if let Ok(remainder) = TryInto::<i64>::try_into(&remainder) {
            // Compute the new degree.
            variable_form_rhs *= remainder;
        }

        if variable_form_rhs == 1 {
            None
        } else {
            Some(Cardinality::from_lits(std::mem::take(&mut self.lits), variable_form_rhs).into())
        }
    }

    #[inline]
    fn weaken(&mut self, var_idx: VarIdx) -> Option<PBConstraintEnum> {
        if self.delete_term(var_idx).is_zero() {
            None
        } else {
            let mut card = Cardinality::from(&*self);
            card.set_degree(0);
            Some(card.into())
        }
    }

    #[inline]
    fn weaken_all(&mut self, mut var_idxs: Vec<VarIdx>) -> Option<PBConstraintEnum> {
        let mut new_degree = 1;
        if var_idxs.len() <= WEAKEN_ALL_THRESHOLD {
            // Weaken all variables in var_idxs by calling the remove function
            for var_idx in var_idxs.into_iter() {
                if !self.delete_term(var_idx).is_zero() {
                    new_degree -= 1;
                }
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
                        new_degree -= 1;
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

        if new_degree == 1 {
            None
        } else {
            let mut card = Cardinality::from(&*self);
            card.set_degree(new_degree);
            Some(card.into())
        }
    }

    #[inline]
    fn multiply(&mut self, factor: &BigInt) -> Option<PBConstraintEnum> {
        if factor.is_one() {
            return None;
        }

        let coeff_sum = factor * self.len();

        // Try i64 type.
        if let (Ok(coeff_sum), Ok(factor)) = (
            TryInto::<i64>::try_into(&coeff_sum),
            TryInto::<i64>::try_into(factor),
        ) {
            let terms = self
                .lits
                .iter()
                .map(|lit| GeneralPBTerm::new(factor, *lit))
                .collect();
            return Some(GeneralPBConstraint::from_terms(terms, coeff_sum, factor).into());
        }

        if let (Ok(coeff_sum), Ok(factor)) = (
            TryInto::<i128>::try_into(&coeff_sum),
            TryInto::<i128>::try_into(factor),
        ) {
            let terms = self
                .lits
                .iter()
                .map(|lit| GeneralPBTerm::new(factor, *lit))
                .collect();
            return Some(GeneralPBConstraint::from_terms(terms, coeff_sum, factor).into());
        }

        let terms = self
            .lits
            .iter()
            .map(|lit| GeneralPBTerm::new(factor.clone(), *lit))
            .collect();
        Some(GeneralPBConstraint::from_terms(terms, coeff_sum, factor.clone()).into())
    }

    fn add<C: PBConstraintGetter>(&mut self, summand: &C) -> Option<PBConstraintEnum> {
        let mut clause_term = self.lits.iter().peekable();
        let mut summand_term = summand.get_terms().iter().peekable();
        let mut cancel = 0;
        let mut resulting_lits = Vec::with_capacity(self.len() + summand.len());

        // Try to keep `Clause` type first.
        merge_in_situ_lits(
            &mut clause_term,
            &mut summand_term,
            &mut resulting_lits,
            &mut cancel,
        );

        // If all terms where merged then it is either a clause or cardinality constraint.
        if clause_term.peek().is_none() && summand_term.peek().is_none() {
            if let Ok(degree) = TryInto::<i64>::try_into(summand.get_degree().to_owned()) {
                if let Some(degree) = degree.checked_add(1).and_then(|d| d.checked_sub(cancel)) {
                    if degree == 1 {
                        self.lits = resulting_lits;
                        return None;
                    } else {
                        return Some(Cardinality::from_lits(resulting_lits, degree).into());
                    }
                }
            }
        }

        // Change clause to general PB constraint.
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

            merge_from_lits(clause_term, summand_term, &mut resulting_terms, &mut cancel);
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

            merge_from_lits(clause_term, summand_term, &mut resulting_terms, &mut cancel);
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

            merge_from_lits(clause_term, summand_term, &mut resulting_terms, &mut cancel);
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
            Some(Cardinality::from_lits(std::mem::take(&mut self.lits), 1 - amount).into())
        } else {
            GeneralPBConstraint::<BigInt>::from(&*self).lower_rhs(amount)
        }
    }

    #[inline]
    fn negate(&self) -> PBConstraintEnum {
        let mut negated_lits = self.lits.clone();
        for lit in negated_lits.iter_mut() {
            lit.negate();
        }

        Cardinality::from_lits(negated_lits, self.get_coeff_sum()).into()
    }

    #[inline]
    fn substitute(&self, substitution: &impl Substitutable) -> PBConstraintEnum {
        let mut substituted_lits = Vec::with_capacity(self.len());
        let mut degree = 1;
        for &lit in self.lits.iter() {
            match substitution.get_lit(lit) {
                Some(SubstitutionValue::TRUE) => degree = 0,
                Some(SubstitutionValue::FALSE) => {}
                Some(substituted_lit) => {
                    substituted_lits.push(GeneralPBTerm::new(1i64, substituted_lit.get_lit()));
                }
                None => substituted_lits.push(GeneralPBTerm::new(1i64, lit)),
            }
        }

        let coeff_sum = substituted_lits.len() as i64;
        constraint_from_terms_and_coeff_sum(substituted_lits, degree, coeff_sum)
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
        for &lit in self.lits.iter() {
            if unsafe { assignment.get_lit_value_unchecked(lit) } == BoolValue::Assigned(true) {
                return true;
            }
        }

        false
    }

    #[inline]
    fn is_falsified(&self, assignment: &Assignment<BooleanVar>) -> bool {
        for &lit in self.lits.iter() {
            if assignment.get_lit_value(lit) != BoolValue::Assigned(false) {
                return false;
            }
        }

        true
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
        let mut unassigned_lit = None;

        for &lit in self.lits.iter() {
            match unsafe { assignment.get_lit_value_unchecked(lit) } {
                BoolValue::Assigned(true) => return ConstraintPropagationResult::NoPropagation,
                BoolValue::Assigned(false) => {}
                BoolValue::Unassigned => {
                    if unassigned_lit.is_some() {
                        return ConstraintPropagationResult::NoPropagation;
                    }
                    unassigned_lit = Some(lit);
                }
            }
        }

        if let Some(lit) = unassigned_lit {
            unsafe { assignment.set_lit_value_unchecked(lit, BoolValue::Assigned(true)) };
            ConstraintPropagationResult::Propagated
        } else {
            ConstraintPropagationResult::Conflict
        }
    }

    #[inline]
    fn traced_propagate(&self, assignment: &mut Assignment<BooleanVar>) -> Vec<Lit> {
        let mut unassigned_lit = None;

        for &lit in self.lits.iter() {
            match unsafe { assignment.get_lit_value_unchecked(lit) } {
                BoolValue::Assigned(true) => return vec![],
                BoolValue::Assigned(false) => {}
                BoolValue::Unassigned => {
                    if unassigned_lit.is_some() {
                        return vec![];
                    }
                    unassigned_lit = Some(lit);
                }
            }
        }

        if let Some(lit) = unassigned_lit {
            unsafe { assignment.set_lit_value_unchecked(lit, BoolValue::Assigned(true)) };
            vec![lit]
        } else {
            panic!("This did not propagate to conflict earlier!")
        }
    }

    #[inline]
    fn mark_negated_lits(
        &self,
        _assignment: &Assignment<BooleanVar>,
        marking: &mut Assignment<BooleanVar>,
    ) {
        for &lit in self.lits.iter() {
            unsafe { marking.set_lit_value_unchecked(lit, BoolValue::Assigned(false)) };
        }
    }

    #[inline]
    fn into_smallest_type(self) -> PBConstraintEnum {
        self.into()
    }
}

impl ToPrettyString for Clause {
    #[inline]
    fn to_pretty_string(&self, var_names: &VarNameManager) -> String {
        let mut out = String::with_capacity(self.len() * 4);
        for &lit in self.lits.iter() {
            out.push_str("1 ");
            out.push_str(&lit.to_pretty_string(var_names));
            out.push(' ');
        }
        out.push_str(">= 1");
        out
    }
}
