use std::{cell::RefCell, rc::Rc};

use ahash::{HashMap, HashMapExt};
use veripb_formula::prelude::*;

use crate::{
    trail::{Propagation, Reason},
    watcher::{LevelZeroPropagationResult, WatchInit, WatchUpdate, Watcher},
};

/// The basic idea for watching general PB constraints is that the sum of the coefficients of the watched terms is at least the degree + largest coefficient.
#[derive(Debug)]
pub struct GeneralPBWatcher<N: Int> {
    constraint: Rc<DBConstraint>,
    watches: HashMap<Lit, N>,
    watched_but_not_contributing: Vec<Lit>,
    pub watches_slack: N,
    pub max_coeff: N,
    pub last_pos: usize,
    is_fully_propagated_cached: Option<bool>,
}

impl<N: Int> GeneralPBWatcher<N> {
    pub fn from_constraint(constraint: GeneralPBConstraint<N>) -> Self
    where
        PBConstraintEnum: From<GeneralPBConstraint<N>>,
    {
        let max_coeff = constraint
            .get_terms()
            .iter()
            .max_by(|term1, term2| term1.get_coeff().cmp(term2.get_coeff()))
            .unwrap()
            .get_coeff()
            .to_owned();
        let slack_init = -constraint.get_degree().clone();
        GeneralPBWatcher {
            constraint: Rc::new(DBConstraint {
                header: RefCell::new(DBHeader::default()),
                constraint: constraint.into(),
            }),
            watches: HashMap::new(),
            watched_but_not_contributing: vec![],
            watches_slack: slack_init.clone(),
            max_coeff,
            last_pos: 0,
            is_fully_propagated_cached: None,
        }
    }
}

impl<N: Int> GeneralPBWatcher<N> {}

impl<N: Int> Watcher for GeneralPBWatcher<N>
where
    PBConstraintEnum: From<GeneralPBConstraint<N>>,
{
    #[inline]
    fn from_db_constraint(constraint: &Rc<DBConstraint>) -> Self {
        let pb_constraint = constraint.constraint.as_general_pb::<N>();
        if let Some(max_term) = pb_constraint
            .get_terms()
            .iter()
            .max_by(|term1, term2| term1.get_coeff().cmp(term2.get_coeff()))
        {
            GeneralPBWatcher {
                constraint: constraint.clone(),
                watches: HashMap::new(),
                watched_but_not_contributing: vec![],
                watches_slack: -pb_constraint.get_degree().clone(),
                max_coeff: max_term.get_coeff().to_owned(),
                last_pos: 0,
                is_fully_propagated_cached: None,
            }
        } else {
            GeneralPBWatcher {
                constraint: constraint.clone(),
                watches: HashMap::new(),
                watched_but_not_contributing: vec![],
                watches_slack: -pb_constraint.get_degree().clone(),
                max_coeff: N::zero(),
                last_pos: 0,
                is_fully_propagated_cached: None,
            }
        }
    }

    #[inline]
    fn get_assignment_independent_propagations(&self) -> LevelZeroPropagationResult {
        let pb_constraint = self.constraint.constraint.as_general_pb::<N>();

        let slack = pb_constraint.get_coeff_sum() - pb_constraint.get_degree();
        let mut propagations = Vec::new();
        for term in pb_constraint.get_terms() {
            if term.get_coeff() > &slack {
                propagations.push(Propagation::new(
                    term.get_lit(),
                    Reason::Constraint(self.constraint.clone()),
                ));
            }
        }

        if propagations.is_empty() {
            LevelZeroPropagationResult::None
        } else if propagations.len() == pb_constraint.len() {
            LevelZeroPropagationResult::Complete(propagations)
        } else {
            LevelZeroPropagationResult::Partial(propagations)
        }
    }

    #[inline]
    fn no_watches_required(&mut self) -> bool {
        if let Some(cached_result) = self.is_fully_propagated_cached {
            return cached_result;
        }
        let pb_constraint = self.constraint.constraint.as_general_pb::<N>();

        let slack = pb_constraint.get_coeff_sum() - pb_constraint.get_degree();
        if !slack.is_positive() || self.constraint.is_trivial() {
            self.is_fully_propagated_cached = Some(true);
            return true;
        }
        for term in pb_constraint.get_terms() {
            if term.get_coeff() <= &slack {
                self.is_fully_propagated_cached = Some(false);
                return false;
            }
        }

        self.is_fully_propagated_cached = Some(true);
        true
    }

    #[inline]
    fn is_propagating_independent_of_assignment(&self) -> bool {
        let pb_constraint = self.constraint.constraint.as_general_pb::<N>();

        let slack = pb_constraint.get_coeff_sum() - pb_constraint.get_degree();
        if !slack.is_positive() {
            return true;
        }
        for term in pb_constraint.get_terms() {
            if term.get_coeff() > &slack {
                return true;
            }
        }

        false
    }

    #[inline]
    fn init_watches(&mut self, assign: &Assignment<BooleanVar>) -> WatchInit {
        if !self.watches.is_empty() {
            // Watches already defined.
            return WatchInit::Watching {
                propagated: vec![],
                watches: vec![],
            };
        }
        let pb_constraint = self.constraint.constraint.as_general_pb::<N>();
        if pb_constraint.is_trivial() {
            return WatchInit::Watching {
                propagated: vec![],
                watches: vec![],
            };
        }

        let terms = pb_constraint.get_terms();
        for (pos, term) in terms.iter().enumerate() {
            let value = unsafe { assign.get_lit_value_unchecked(term.get_lit()) };
            match value {
                BoolValue::Unassigned | BoolValue::Assigned(true) => {
                    self.watches.insert(term.get_lit(), term.coeff.clone());
                    self.watches_slack += term.get_coeff();
                    if self.watches_slack >= self.max_coeff {
                        self.last_pos = pos + 1;
                        return WatchInit::Watching {
                            propagated: vec![],
                            watches: self.watches.iter().map(|(&l, _)| l).collect(),
                        };
                    }
                }
                BoolValue::Assigned(false) => {}
            }
        }

        // If slack is negative, then constraint is conflicting.
        if self.watches_slack.is_negative() {
            self.watches.clear();
            self.watches_slack = -pb_constraint.get_degree().clone();
            return WatchInit::Conflict;
        }

        // Check which literals are propagated.
        let mut propagated = Vec::new();
        for (&lit, coeff) in self.watches.iter() {
            if coeff > &self.watches_slack {
                propagated.push(lit);
            }
        }

        // Find enough literals to watch if we reset the propagator.
        let mut temp_slack = self.watches_slack.clone();
        for term in terms.iter() {
            let value = unsafe { assign.get_lit_value_unchecked(term.get_lit()) };
            match value {
                BoolValue::Unassigned | BoolValue::Assigned(true) => {}
                BoolValue::Assigned(false) => {
                    self.watched_but_not_contributing.push(term.get_lit());
                    temp_slack += term.get_coeff();
                    if temp_slack >= self.max_coeff {
                        break;
                    }
                }
            }
        }

        let mut watches: Vec<_> = self.watches.iter().map(|(&l, _)| l).collect();
        watches.extend(self.watched_but_not_contributing.iter());
        WatchInit::Watching {
            propagated,
            watches,
        }
    }

    #[inline]
    fn update_watch(&mut self, old_watch: Lit, assign: &Assignment<BooleanVar>) -> WatchUpdate {
        let pb_constraint = self.constraint.constraint.as_general_pb::<N>();

        let original_slack = self.watches_slack.clone();

        // Remove old watch.
        if let Some(coeff) = self.watches.remove(&old_watch) {
            self.watches_slack -= coeff;
        } else {
            let index = self
                .watched_but_not_contributing
                .iter()
                .position(|&lit| lit == old_watch)
                .expect("literal should be watched");
            self.watched_but_not_contributing.swap_remove(index);
        }

        // Maybe we still watch enough literals.
        if self.watches_slack >= self.max_coeff {
            return WatchUpdate::None;
        }

        // Find new watches.
        let mut added_watch_coeff_pairs = Vec::new();
        for term in pb_constraint.get_terms()[self.last_pos..].iter() {
            self.last_pos += 1;
            let lit = term.get_lit();
            let value = unsafe { assign.get_lit_value_unchecked(lit) };
            match value {
                BoolValue::Unassigned | BoolValue::Assigned(true) => {
                    if !self.watches.contains_key(&lit) {
                        added_watch_coeff_pairs.push((term.lit, term.coeff.clone()));
                        self.watches_slack += term.get_coeff().to_owned();
                        if self.watches_slack >= self.max_coeff {
                            let added_watches =
                                added_watch_coeff_pairs.iter().map(|&(l, _)| l).collect();
                            self.watches.extend(added_watch_coeff_pairs);
                            return WatchUpdate::Unknown {
                                propagated: vec![],
                                new_watches: added_watches,
                            };
                        }
                    }
                }
                BoolValue::Assigned(false) => {}
            }
        }
        self.last_pos = pb_constraint.len();

        // Detect conflict.
        if self.watches_slack.is_negative() {
            self.watches_slack = original_slack;
            self.watches.insert(
                old_watch,
                pb_constraint.get_coeff(old_watch).unwrap().clone(),
            );
            return WatchUpdate::Conflict;
        }

        // Check for propagations.
        let mut added_watches: Vec<_> = added_watch_coeff_pairs.iter().map(|&(l, _)| l).collect();
        self.watches.extend(added_watch_coeff_pairs);
        let mut propagated = Vec::new();
        for (&lit, coeff) in self.watches.iter() {
            if coeff > &self.watches_slack {
                propagated.push(lit);
            }
        }

        // Could not find enough watches so that the watch slack is at least the largest coefficient. Keep old watch to watch enough after reset.
        added_watches.push(old_watch);
        // The old watch is no longer contributing to the slack of the watches and should be recovered later.
        self.watched_but_not_contributing.push(old_watch);

        WatchUpdate::Unknown {
            propagated,
            new_watches: added_watches,
        }
    }

    #[inline]
    fn get_watches(&self) -> impl Iterator<Item = &Lit> {
        self.watches
            .keys()
            .chain(self.watched_but_not_contributing.iter())
    }

    #[inline]
    fn get_constraint(&self) -> &Rc<DBConstraint> {
        &self.constraint
    }

    #[inline]
    fn reset_last_pos(&mut self) {
        self.last_pos = 0;
    }

    #[inline]
    fn increase_slack(&mut self, lit: Lit) {
        if let Some(index) = self
            .watched_but_not_contributing
            .iter()
            .position(|l| l == &lit)
        {
            let pb_constraint = self.constraint.constraint.as_general_pb::<N>();

            let coeff = pb_constraint.get_coeff(lit).unwrap();
            self.watches_slack += pb_constraint.get_coeff(lit).unwrap();
            self.watched_but_not_contributing.swap_remove(index);
            self.watches.insert(lit, coeff.clone());
        }
    }
}
