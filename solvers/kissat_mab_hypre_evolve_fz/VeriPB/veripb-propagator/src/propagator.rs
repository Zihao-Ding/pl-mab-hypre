use std::rc::Rc;

use veripb_formula::prelude::*;

use crate::{
    error::PropagatorError,
    trail::{Propagation, Reason, Trail},
    watcher::{LevelZeroPropagationResult, WatchInit, WatchUpdate, Watcher},
};

#[derive(Debug, PartialEq, Eq)]
pub enum PropagationResult {
    Unknown,
    Conflict,
}

impl PropagationResult {
    /// Check if the `PropagationResult` is a conflict or not.
    #[inline]
    pub fn is_conflict(&self) -> bool {
        matches!(*self, PropagationResult::Conflict)
    }
}

#[derive(Debug)]
pub struct Propagator<W>
where
    W: Watcher,
{
    watchlist: Vec<Vec<Rc<DBConstraint>>>,
    watchers: Vec<Option<Box<W>>>,
    empty_watcher_pos: usize,
    unregistered_watchers: Vec<usize>,
    trail_head: usize,
    /// A list of propagation that will always happen. E.g., this are unit clauses or cardinality constraints with a degree equal to the number of literals in the constraint.
    assignment_independent_propagations: Vec<Propagation>,
    unhandled_assignment_independent_watchers: Vec<usize>,
    touched_watchers: Vec<usize>,
}

impl<W> Propagator<W>
where
    W: Watcher,
{
    /// Create a new propagator with capacity for specified variables.
    #[inline]
    pub fn with_size(num_variables: usize) -> Self {
        Propagator {
            watchlist: vec![Vec::new(); num_variables * 2],
            watchers: Vec::new(),
            empty_watcher_pos: 0,
            unregistered_watchers: Vec::new(),
            trail_head: 0,
            assignment_independent_propagations: Vec::new(),
            unhandled_assignment_independent_watchers: Vec::new(),
            touched_watchers: Vec::new(),
        }
    }

    /// Resize the propagator to fit a size of `num_variable` many variables. Since the watchlist is indexed by the literal, we need to allocate twice the number of variables.
    #[inline]
    pub fn resize(&mut self, num_variables: usize) {
        self.watchlist.resize(num_variables * 2, Vec::new());
    }

    #[inline]
    fn register_watches(&mut self, new_watches: &[Lit], constraint: &Rc<DBConstraint>) {
        for lit in new_watches {
            self.watchlist[lit.get_lit_data()].push(constraint.clone());
        }
    }

    #[inline]
    fn register_watch(&mut self, new_watch: Lit, constraint: &Rc<DBConstraint>) {
        self.watchlist[new_watch.get_lit_data()].push(constraint.clone());
    }

    /// Add `constraint` to the propagator.
    ///
    /// The propagator should create a watcher to watch the constraint and add the watcher to the watchers handled by the propagator. This operation should not add watches to the watchlist of the propagator.
    #[inline]
    pub fn add(&mut self, constraint: &Rc<DBConstraint>) -> Result<(), PropagatorError> {
        if constraint.header.borrow().propagator_id.is_some() {
            return Err(PropagatorError::AttachingAttached);
        }
        let watcher = W::from_db_constraint(constraint);
        for watcher in self.watchers[self.empty_watcher_pos..].iter() {
            if watcher.is_none() {
                break;
            }
            self.empty_watcher_pos += 1;
        }
        constraint.header.borrow_mut().propagator_id = Some(self.empty_watcher_pos);
        if self.empty_watcher_pos == self.watchers.len() {
            self.watchers.push(Some(Box::new(watcher)));
        } else {
            // We checked that the index is inside the size of the vector.
            *unsafe { self.watchers.get_unchecked_mut(self.empty_watcher_pos) } =
                Some(Box::new(watcher));
        }
        self.unregistered_watchers.push(self.empty_watcher_pos);
        self.unhandled_assignment_independent_watchers
            .push(self.empty_watcher_pos);
        self.empty_watcher_pos += 1;
        Ok(())
    }

    /// Remove `constraint` from the propagator.
    ///
    /// Remove the constraint from the propagator and delete the constraint from the watchlist, as there is no easy way to remove the constraint lazily. This is because the constraint could immediately be added to a different propagator.
    #[inline]
    pub fn remove(&mut self, constraint: &Rc<DBConstraint>) -> Result<(), PropagatorError> {
        let mut header = constraint.header.borrow_mut();
        if let Some(propagator_id) = header.propagator_id {
            if let Some(mut watcher) = self.watchers[propagator_id].take() {
                // Track first free space.
                if propagator_id < self.empty_watcher_pos {
                    self.empty_watcher_pos = propagator_id;
                }
                header.propagator_id = None;
                if watcher.is_propagating_independent_of_assignment() {
                    let reason = Reason::Constraint(constraint.clone());
                    self.assignment_independent_propagations
                        .retain(|p| !Rc::ptr_eq(p.reason.unwrap(), reason.unwrap()));
                    if watcher.no_watches_required() {
                        return Ok(());
                    }
                }
                // Watches are removed eagerly.
                for lit in watcher.get_watches() {
                    if lit.is_undef() {
                        continue;
                    }
                    let list = self.watchlist.get_mut(lit.get_lit_data()).unwrap();
                    let index = list.iter().position(|c| Rc::ptr_eq(c, constraint)).unwrap();
                    list.swap_remove(index);
                }

                Ok(())
            } else {
                Err(PropagatorError::AttachedToOtherPropagator)
            }
        } else {
            Ok(())
        }
    }

    /// Check if propagtor has a constraint that is a reason in the saved trail.
    pub fn has_saved_reason(&self) -> bool {
        for watcher in self.watchers.iter().flatten() {
            if watcher.get_constraint().header.borrow().is_saved_reason {
                return true;
            }
        }
        false
    }

    /// Remove all constraints from this propagator and return the removed constraints.
    pub fn remove_all(&mut self, removed_constraints: &mut Vec<Rc<DBConstraint>>) {
        for watcher in self.watchers.iter().flatten() {
            let constraint = watcher.get_constraint();
            constraint.header.borrow_mut().propagator_id = None;
            removed_constraints.push(constraint.clone());
        }

        for list in self.watchlist.iter_mut() {
            list.clear();
        }
        self.watchers.clear();
        self.assignment_independent_propagations.clear();
        self.empty_watcher_pos = 0;
        self.unregistered_watchers.clear();
        self.unhandled_assignment_independent_watchers.clear();
        self.trail_head = 0;
        self.touched_watchers.clear();
    }

    /// Reset the propagator to the assignment.
    #[inline]
    pub fn reset(&mut self, prev_trail_len: usize) {
        self.trail_head = prev_trail_len;
    }

    /// Register unregistered constraints to the watchlist.
    #[inline]
    fn register_unregistered_to_watchlist(
        &mut self,
        trail: &mut Trail,
        mark_reasons: bool,
        remember_watcher: bool,
    ) -> Option<PropagationResult> {
        while let Some(watcher_pos) = self.unregistered_watchers.pop() {
            if let Some(watcher) = self.watchers.get_mut(watcher_pos).unwrap() {
                // Do not add watches for constraints that are already fully propagated.
                if watcher.no_watches_required() {
                    // Check if the constraint is also conflicting.
                    if watcher.get_constraint().is_contradicting() {
                        // Directly terminate with conflict, if constraint is conflicting.
                        trail.conflict = Some(Reason::Constraint(watcher.get_constraint().clone()));
                        if mark_reasons {
                            watcher.get_constraint().header.borrow_mut().is_saved_reason = true;
                        }
                        self.unregistered_watchers.push(watcher_pos);
                        return Some(PropagationResult::Conflict);
                    }
                    continue;
                }

                if remember_watcher {
                    self.touched_watchers.push(watcher_pos);
                }
                match watcher.init_watches(&trail.assignment) {
                    WatchInit::Conflict => {
                        trail.conflict = Some(Reason::Constraint(watcher.get_constraint().clone()));
                        if mark_reasons {
                            watcher.get_constraint().header.borrow_mut().is_saved_reason = true;
                        }
                        self.unregistered_watchers.push(watcher_pos);
                        return Some(PropagationResult::Conflict);
                    }
                    WatchInit::Watching {
                        propagated,
                        watches,
                    } => {
                        // Initialize watches for unregistered constraint.
                        let constraint = watcher.get_constraint().clone();
                        self.register_watches(&watches, &constraint);
                        for &lit in propagated.iter() {
                            if trail
                                .push(
                                    Propagation::new(lit, Reason::Constraint(constraint.clone())),
                                    mark_reasons,
                                )
                                .is_err()
                            {
                                return Some(PropagationResult::Conflict);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Propagate using `trail`.
    #[inline]
    pub fn propagate(
        &mut self,
        trail: &mut Trail,
        mark_reasons: bool,
        remember_watcher: bool,
    ) -> PropagationResult {
        if self.watchers.is_empty() {
            return PropagationResult::Unknown;
        }
        if let Some(value) =
            self.register_unregistered_to_watchlist(trail, mark_reasons, remember_watcher)
        {
            return value;
        }

        // Update watches for the negated literals on the trail.
        while self.trail_head < trail.len() {
            let Propagation { mut lit, reason: _ } = trail.trail[self.trail_head];
            lit.negate();
            let list = unsafe { self.watchlist.get_unchecked_mut(lit.get_lit_data()) };
            if list.is_empty() {
                self.trail_head += 1;
                continue;
            }
            let mut list = std::mem::take(list);
            while let Some(constraint) = list.pop() {
                let propagator_id =
                    unsafe { constraint.header.borrow().propagator_id.unwrap_unchecked() };
                let watcher = unsafe {
                    self.watchers
                        .get_unchecked_mut(propagator_id)
                        .as_mut()
                        .unwrap_unchecked()
                };
                if remember_watcher {
                    self.touched_watchers.push(propagator_id);
                }
                match watcher.update_watch(lit, &trail.assignment) {
                    WatchUpdate::None => {}
                    WatchUpdate::Conflict => {
                        trail.conflict = Some(Reason::Constraint(constraint.clone()));
                        if mark_reasons {
                            constraint.header.borrow_mut().is_saved_reason = true;
                        }
                        self.watchlist[lit.get_lit_data()].push(constraint);
                        self.watchlist[lit.get_lit_data()].append(&mut list);
                        return PropagationResult::Conflict;
                    }
                    WatchUpdate::Unknown {
                        propagated,
                        new_watches,
                    } => {
                        self.register_watches(&new_watches, &constraint);
                        for &propagated_lit in propagated.iter() {
                            if trail
                                .push(
                                    Propagation {
                                        lit: propagated_lit,
                                        reason: Reason::Constraint(constraint.clone()),
                                    },
                                    mark_reasons,
                                )
                                .is_err()
                            {
                                self.watchlist[lit.get_lit_data()].append(&mut list);
                                return PropagationResult::Conflict;
                            }
                        }
                    }
                    WatchUpdate::UnknownSingle {
                        propagated,
                        new_watches,
                    } => {
                        self.register_watch(new_watches, &constraint);
                        if let Some(propagated_lit) = propagated {
                            if trail
                                .push(
                                    Propagation {
                                        lit: propagated_lit,
                                        reason: Reason::Constraint(constraint.clone()),
                                    },
                                    mark_reasons,
                                )
                                .is_err()
                            {
                                self.watchlist[lit.get_lit_data()].append(&mut list);
                                return PropagationResult::Conflict;
                            }
                        }
                    }
                }
            }

            self.trail_head += 1;
        }

        PropagationResult::Unknown
    }

    #[inline]
    pub fn get_assignment_independent_propagations(&mut self) -> &Vec<Propagation> {
        // Check for propagation at level zero of new watchers.
        while let Some(watcher_pos) = self.unhandled_assignment_independent_watchers.pop() {
            if let Some(watcher) = &self.watchers[watcher_pos] {
                match &mut watcher.get_assignment_independent_propagations() {
                    LevelZeroPropagationResult::Partial(propagations) => {
                        self.assignment_independent_propagations
                            .append(propagations);
                    }
                    LevelZeroPropagationResult::Complete(propagations) => {
                        self.assignment_independent_propagations
                            .append(propagations);
                    }
                    LevelZeroPropagationResult::None => {}
                }
            }
        }

        &self.assignment_independent_propagations
    }

    #[inline]
    pub fn get_new_assignment_independent_propagations(
        &mut self,
        new_propagations: &mut Vec<Propagation>,
    ) {
        // Check for propagation at level zero of new watchers.
        while let Some(watcher_pos) = self.unhandled_assignment_independent_watchers.pop() {
            if let Some(watcher) = &self.watchers[watcher_pos] {
                match &mut watcher.get_assignment_independent_propagations() {
                    LevelZeroPropagationResult::Partial(propagations) => {
                        new_propagations.extend(propagations.iter().cloned());
                        self.assignment_independent_propagations
                            .append(propagations);
                    }
                    LevelZeroPropagationResult::Complete(propagations) => {
                        new_propagations.extend(propagations.iter().cloned());
                        self.assignment_independent_propagations
                            .append(propagations);
                    }
                    LevelZeroPropagationResult::None => {}
                }
            }
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.watchers.drain(..).flatten().for_each(|watcher| {
            watcher.get_watches().for_each(|lit| {
                let opt_watchlist = self.watchlist.get_mut(lit.get_lit_data());
                if let Some(watchlist) = opt_watchlist {
                    watchlist.clear()
                }
            });
            watcher.get_constraint().header.borrow_mut().propagator_id = None;
        });
        self.watchers.clear();
        self.empty_watcher_pos = 0;
        self.unregistered_watchers.clear();
        self.trail_head = 0;
        self.assignment_independent_propagations.clear();
        self.unhandled_assignment_independent_watchers.clear();
        self.touched_watchers.clear();
    }

    #[inline]
    pub fn reset_last_pos(&mut self) {
        if self.watchers.len() < self.touched_watchers.len() {
            self.watchers
                .iter_mut()
                .flatten()
                .for_each(|w| w.reset_last_pos());
        } else {
            self.touched_watchers.iter().for_each(|&pos| {
                if let Some(watcher) = unsafe { self.watchers.get_unchecked_mut(pos) } {
                    watcher.reset_last_pos();
                }
            })
        }
        self.touched_watchers.clear();
    }

    #[inline]
    pub fn increase_slack(&mut self, mut lit: Lit) {
        lit.negate();
        for constraint in unsafe { self.watchlist.get_unchecked(lit.get_lit_data()).iter() } {
            let watcher = unsafe {
                self.watchers
                    .get_unchecked_mut(constraint.header.borrow().propagator_id.unwrap_unchecked())
                    .as_mut()
                    .unwrap_unchecked()
            };
            watcher.increase_slack(lit);
        }
    }
}

impl<W: Watcher> Default for Propagator<W> {
    fn default() -> Self {
        Propagator {
            watchlist: Vec::new(),
            watchers: Vec::new(),
            empty_watcher_pos: 0,
            unregistered_watchers: Vec::new(),
            trail_head: 0,
            assignment_independent_propagations: Vec::new(),
            unhandled_assignment_independent_watchers: Vec::new(),
            touched_watchers: Vec::new(),
        }
    }
}
