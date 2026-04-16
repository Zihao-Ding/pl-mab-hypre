use std::{cell::RefCell, ops::ControlFlow, rc::Rc};

use veripb_formula::prelude::*;

use crate::{
    trail::{Propagation, Reason},
    watcher::{LevelZeroPropagationResult, WatchInit, WatchUpdate, Watcher},
};

#[derive(Debug)]
pub struct ClauseWatcher {
    pub constraint: Rc<DBConstraint>,
    pub watches: [Lit; 2],
    /// Position in the literals of the clause to next look for a watch.
    pub next_pos: usize,
}

impl ClauseWatcher {
    pub fn from_clause(clause: Clause) -> Self {
        ClauseWatcher {
            constraint: Rc::new(DBConstraint {
                header: RefCell::new(DBHeader::default()),
                constraint: clause.into(),
            }),
            watches: [Lit::new_undef(), Lit::new_undef()],
            next_pos: 0,
        }
    }
}

impl Watcher for ClauseWatcher {
    #[inline]
    fn from_db_constraint(constraint: &Rc<DBConstraint>) -> Self {
        ClauseWatcher {
            constraint: constraint.clone(),
            watches: [Lit::new_undef(), Lit::new_undef()],
            next_pos: 0,
        }
    }

    #[inline]
    fn get_assignment_independent_propagations(&self) -> LevelZeroPropagationResult {
        let clause = self.constraint.constraint.as_clause();
        if clause.len() == 1 {
            LevelZeroPropagationResult::Complete(vec![Propagation::new(
                *clause.get_lit(0usize).unwrap(),
                Reason::Constraint(self.constraint.clone()),
            )])
        } else {
            LevelZeroPropagationResult::None
        }
    }

    #[inline]
    fn no_watches_required(&mut self) -> bool {
        let clause = self.constraint.constraint.as_clause();
        clause.len() <= 1
    }

    #[inline]
    fn is_propagating_independent_of_assignment(&self) -> bool {
        let clause = self.constraint.constraint.as_clause();
        clause.len() <= 1
    }

    #[inline]
    fn init_watches(&mut self, assign: &Assignment<BooleanVar>) -> WatchInit {
        if !self.watches[1].is_undef() {
            // Watches already defined.
            return WatchInit::Watching {
                propagated: vec![],
                watches: vec![],
            };
        }
        let clause = self.constraint.constraint.as_clause();

        let mut iter = clause.get_lits().enumerate();

        let result1 = iter.try_fold((), |_, (n, &lit)| {
            let value = unsafe { assign.get_lit_value_unchecked(lit) };
            match value {
                BoolValue::Assigned(true) => ControlFlow::Break((true, n, lit)),
                BoolValue::Unassigned => ControlFlow::Break((false, n, lit)),
                BoolValue::Assigned(false) => ControlFlow::Continue(()),
            }
        });

        match result1 {
            ControlFlow::Continue(()) => WatchInit::Conflict,
            ControlFlow::Break((true, n, lit)) => {
                self.next_pos = n + 1usize;
                self.watches[0] = lit;
                self.watches[1] = {
                    let pos = (n == 0usize) as usize;
                    *clause.get_lit(pos).unwrap()
                };
                WatchInit::Watching {
                    propagated: vec![],
                    watches: vec![self.watches[0], self.watches[1]],
                }
            }
            ControlFlow::Break((false, n1, lit1)) => {
                self.watches[0] = lit1;
                let result2 = iter.try_fold((), |_, (n, &lit)| {
                    let value = unsafe { assign.get_lit_value_unchecked(lit) };
                    match value {
                        BoolValue::Assigned(false) => ControlFlow::Continue(()),
                        _ => ControlFlow::Break((n, lit)),
                    }
                });
                match result2 {
                    ControlFlow::Continue(_) => {
                        self.watches[1] = {
                            let pos = (n1 == 0usize) as usize;
                            *clause.get_lit(pos).unwrap()
                        };
                        self.next_pos = n1 + 1usize;
                        WatchInit::Watching {
                            propagated: vec![self.watches[0]],
                            watches: vec![self.watches[0], self.watches[1]],
                        }
                    }
                    ControlFlow::Break((n, lit2)) => {
                        self.watches[1] = lit2;
                        self.next_pos = n + 1usize;
                        WatchInit::Watching {
                            propagated: vec![],
                            watches: vec![self.watches[0], self.watches[1]],
                        }
                    }
                }
            }
        }
    }

    #[inline]
    fn update_watch(&mut self, old_watch: Lit, assign: &Assignment<BooleanVar>) -> WatchUpdate {
        debug_assert!(!self.watches[0].is_undef());
        debug_assert!(!self.watches[1].is_undef());
        debug_assert_ne!(self.watches[0], self.watches[1]);

        let other_watch = if old_watch == unsafe { *self.watches.get_unchecked(0) } {
            unsafe { *self.watches.get_unchecked(1) }
        } else {
            unsafe { *self.watches.get_unchecked(0) }
        };

        if self.constraint.constraint.len() == 2 {
            return WatchUpdate::UnknownSingle {
                propagated: Some(other_watch),
                new_watches: old_watch,
            };
        }

        let clause = self.constraint.constraint.as_clause();

        // Find new literal in clause to watch
        let result = {
            let slice = clause.as_slice();
            debug_assert!(self.next_pos <= slice.len());
            let (llits, rlits) = unsafe { slice.split_at_unchecked(self.next_pos) };
            rlits
                .iter()
                .chain(llits.iter().take(self.next_pos - 1))
                .enumerate()
                .try_for_each(|(n, &lit)| {
                    let value = unsafe { assign.get_lit_value_unchecked(lit) };
                    if value != BoolValue::Assigned(false) && lit != other_watch {
                        ControlFlow::Break((n, lit))
                    } else {
                        ControlFlow::Continue(())
                    }
                })
        };
        match result {
            ControlFlow::Continue(_) => {
                let value = unsafe { assign.get_lit_value_unchecked(other_watch) };
                match value {
                    // Clause is propagating and will not propagate again. Hence, we can directly reset it by continuing to watch the `old_watch`.
                    BoolValue::Unassigned => WatchUpdate::UnknownSingle {
                        propagated: Some(other_watch),
                        new_watches: old_watch,
                    },
                    // Clause is satisfied will not propagate again. Hence, we can directly reset it by continuing to watch the `old_watch`.
                    BoolValue::Assigned(true) => WatchUpdate::UnknownSingle {
                        propagated: None,
                        new_watches: old_watch,
                    },
                    BoolValue::Assigned(false) => WatchUpdate::Conflict,
                }
            }
            ControlFlow::Break((n, lit)) => {
                // WARNING: this probably breaks for clauses of size greater than u32::MAX (so probably it's fine)
                self.next_pos = ((self.next_pos + n) % clause.len()) + 1usize;
                if old_watch == unsafe { *self.watches.get_unchecked(0) } {
                    self.watches[0] = lit;
                } else {
                    self.watches[1] = lit;
                }
                WatchUpdate::UnknownSingle {
                    propagated: None,
                    new_watches: lit,
                }
            }
        }
    }

    #[inline]
    fn get_watches(&self) -> impl Iterator<Item = &Lit> {
        self.watches.iter()
    }

    #[inline]
    fn get_constraint(&self) -> &Rc<DBConstraint> {
        &self.constraint
    }
}
