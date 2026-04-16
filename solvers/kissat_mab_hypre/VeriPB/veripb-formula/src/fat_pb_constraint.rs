use malachite_bigint::BigInt;
use num_traits::{One, Zero};

use crate::{
    cardinality::Cardinality,
    clause::Clause,
    general_pb_constraint::GeneralPBConstraint,
    pb_constraint::{DynPBConstraint, Int, PBConstraint},
    pb_term::PBTerm,
    var_type::VarIdx,
};

#[derive(Debug, Clone)]
pub struct FatPBConstraint<N: Int> {
    pub coeffs: Vec<N>,
    pub degree: N,
    pub vars: Vec<VarIdx>,
}

impl<N: Int> FatPBConstraint<N> {
    #[inline]
    pub fn weaken(&mut self, var: VarIdx) {
        if let Ok(idx) = self.vars.binary_search(&var) {
            let coeff = self.coeffs.get_mut(var).unwrap();
            *coeff = coeff.abs();
            self.degree -= coeff.clone();
            coeff.set_zero();
            self.vars.remove(idx);
        }
    }
}

impl<N: Int> From<&Clause> for FatPBConstraint<N> {
    #[inline]
    fn from(value: &Clause) -> Self {
        let mut vars = Vec::with_capacity(value.get_lits().len());
        let mut coeffs: Vec<N> = vec![Zero::zero(); 100];
        for lit in value.get_lits() {
            vars.push(lit.get_var());
            if lit.is_negated() {
                coeffs[lit.get_var()] -= N::one();
            } else {
                coeffs[lit.get_var()] = N::one();
            }
        }
        vars.sort_unstable();
        FatPBConstraint {
            coeffs,
            degree: One::one(),
            vars,
        }
    }
}

impl<N: Int> From<&Cardinality> for FatPBConstraint<N> {
    #[inline]
    fn from(value: &Cardinality) -> Self {
        let mut vars = Vec::with_capacity(value.get_lits().len());
        let mut coeffs: Vec<N> = vec![Zero::zero(); 100];
        for lit in value.get_lits() {
            vars.push(lit.get_var());
            if lit.is_negated() {
                coeffs[lit.get_var()] -= N::one();
            } else {
                coeffs[lit.get_var()] = N::one();
            }
        }
        vars.sort_unstable();
        FatPBConstraint {
            coeffs,
            degree: (*value.get_degree()).into(),
            vars,
        }
    }
}

impl<N: Int + Into<M>, M: Int + From<N>> From<&GeneralPBConstraint<N>> for FatPBConstraint<M> {
    #[inline]
    fn from(value: &GeneralPBConstraint<N>) -> Self {
        let mut vars = Vec::with_capacity(value.get_lits().len());
        let mut coeffs: Vec<M> = vec![Zero::zero(); 100];
        for term in value.get_terms().iter() {
            vars.push(term.get_lit().get_var());
            if term.get_lit().is_negated() {
                coeffs[term.get_lit().get_var()] -= Into::<M>::into(term.get_coeff().to_owned());
            } else {
                coeffs[term.get_lit().get_var()] = term.get_coeff().clone().into();
            }
        }
        vars.sort_unstable();
        FatPBConstraint {
            coeffs,
            degree: (value.get_degree().clone()).into(),
            vars,
        }
    }
}

impl From<&dyn DynPBConstraint> for FatPBConstraint<i64> {
    fn from(value: &dyn DynPBConstraint) -> Self {
        let any = value.as_any();
        if let Some(c) = any.downcast_ref::<Clause>() {
            return c.into();
        }
        if let Some(c) = any.downcast_ref::<Cardinality>() {
            return c.into();
        }
        if let Some(c) = any.downcast_ref::<GeneralPBConstraint<i64>>() {
            return c.into();
        }
        panic!();
    }
}

impl From<&dyn DynPBConstraint> for FatPBConstraint<i128> {
    fn from(value: &dyn DynPBConstraint) -> Self {
        let any = value.as_any();
        if let Some(c) = any.downcast_ref::<Clause>() {
            return c.into();
        }
        if let Some(c) = any.downcast_ref::<Cardinality>() {
            return c.into();
        }
        if let Some(c) = any.downcast_ref::<GeneralPBConstraint<i64>>() {
            return c.into();
        }
        if let Some(c) = any.downcast_ref::<GeneralPBConstraint<i128>>() {
            return c.into();
        }
        panic!();
    }
}

impl From<&dyn DynPBConstraint> for FatPBConstraint<BigInt> {
    fn from(value: &dyn DynPBConstraint) -> Self {
        let any = value.as_any();
        if let Some(c) = any.downcast_ref::<Clause>() {
            return c.into();
        }
        if let Some(c) = any.downcast_ref::<Cardinality>() {
            return c.into();
        }
        if let Some(c) = any.downcast_ref::<GeneralPBConstraint<i64>>() {
            return c.into();
        }
        if let Some(c) = any.downcast_ref::<GeneralPBConstraint<i128>>() {
            return c.into();
        }
        if let Some(c) = any.downcast_ref::<GeneralPBConstraint<BigInt>>() {
            return c.into();
        }
        panic!();
    }
}
