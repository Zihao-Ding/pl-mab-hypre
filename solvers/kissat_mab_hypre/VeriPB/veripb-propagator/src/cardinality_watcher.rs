use std::{cell::RefCell, rc::Rc};

use veripb_formula::prelude::*;

use crate::{
    trail::{Propagation, Reason},
    watcher::{LevelZeroPropagationResult, WatchInit, WatchUpdate, Watcher},
};

#[derive(Debug)]
pub struct CardinalityWatcher {
    pub constraint: Rc<DBConstraint>,
    pub watches: Vec<Lit>,
    pub last_pos: usize,
}

impl CardinalityWatcher {
    pub fn from_card(card: Cardinality) -> Self {
        if card.is_trivial() {
            CardinalityWatcher {
                constraint: Rc::new(DBConstraint {
                    header: RefCell::new(DBHeader::default()),
                    constraint: card.into(),
                }),
                watches: vec![],
                last_pos: 0,
            }
        } else {
            let num_watches = card.len().min(*card.get_degree() as usize + 1);
            CardinalityWatcher {
                constraint: Rc::new(DBConstraint {
                    header: RefCell::new(DBHeader::default()),
                    constraint: card.into(),
                }),
                watches: vec![Lit::new_undef(); num_watches],
                last_pos: 0,
            }
        }
    }
}

impl Watcher for CardinalityWatcher {
    #[inline]
    fn from_db_constraint(constraint: &Rc<DBConstraint>) -> Self {
        let card = constraint.constraint.as_card();
        if card.is_trivial() {
            CardinalityWatcher {
                constraint: constraint.clone(),
                watches: vec![],
                last_pos: 0,
            }
        } else {
            let size = card.len().min(*card.get_degree() as usize + 1);
            CardinalityWatcher {
                constraint: constraint.clone(),
                watches: vec![Lit::new_undef(); size],
                last_pos: 0,
            }
        }
    }

    #[inline]
    fn get_assignment_independent_propagations(&self) -> LevelZeroPropagationResult {
        let card = self.constraint.constraint.as_card();
        if card.len() as i64 == *card.get_degree() {
            let mut propagations = Vec::with_capacity(card.len());
            for lit in card.get_lits() {
                propagations.push(Propagation::new(
                    *lit,
                    Reason::Constraint(self.constraint.clone()),
                ));
            }
            LevelZeroPropagationResult::Complete(propagations)
        } else {
            LevelZeroPropagationResult::None
        }
    }

    #[inline]
    fn no_watches_required(&mut self) -> bool {
        self.is_propagating_independent_of_assignment() || self.constraint.is_trivial()
    }

    #[inline]
    fn is_propagating_independent_of_assignment(&self) -> bool {
        let card = self.constraint.constraint.as_card();
        card.len() as i64 <= *card.get_degree()
    }

    #[inline]
    fn init_watches(&mut self, assign: &Assignment<BooleanVar>) -> WatchInit {
        if !self.watches.last().is_some_and(|l| l.is_undef()) {
            // Watches already defined.
            return WatchInit::Watching {
                propagated: vec![],
                watches: vec![],
            };
        }
        let card = self.constraint.constraint.as_card();
        if card.is_trivial() {
            return WatchInit::Watching {
                propagated: vec![],
                watches: vec![],
            };
        }

        let mut watch_pos = 0;
        for (pos, &lit) in card.get_lits().enumerate() {
            let value = unsafe { assign.get_lit_value_unchecked(lit) };
            match value {
                BoolValue::Unassigned | BoolValue::Assigned(true) => {
                    self.watches[watch_pos] = lit;
                    watch_pos += 1;
                    if watch_pos == self.watches.len() {
                        self.last_pos = pos + 1;
                        return WatchInit::Watching {
                            propagated: vec![],
                            watches: self.watches.clone(),
                        };
                    }
                }
                BoolValue::Assigned(false) => {}
            }
        }

        // If we managed to get watches for all but one of the required watch, we propagated the watches.
        if watch_pos == self.watches.len() - 1 {
            // Assign last remaining watch to an unwatched literal.
            for (pos, &lit) in card.get_lits().enumerate() {
                if lit != self.watches[pos] {
                    *self.watches.last_mut().unwrap() = lit;
                    break;
                }
            }
            WatchInit::Watching {
                propagated: self.watches[..self.watches.len() - 1].to_vec(),
                watches: self.watches.clone(),
            }
        } else {
            for watch in self.watches.iter_mut() {
                *watch = Lit::new_undef();
            }
            WatchInit::Conflict
        }
    }

    fn update_watch(&mut self, old_watch: Lit, assign: &Assignment<BooleanVar>) -> WatchUpdate {
        let card = self.constraint.constraint.as_card();

        debug_assert_eq!(0.max(*card.get_degree()), self.watches.len() as i64 - 1);

        // Remove old watch from watches.
        let index = self
            .watches
            .iter()
            .position(|&lit| lit == old_watch)
            .unwrap();
        self.watches.swap_remove(index);

        let (llits, rlits) = unsafe { card.as_slice().split_at_unchecked(self.last_pos) };
        for (n, &lit) in (rlits.iter()).chain(llits.iter()).enumerate() {
            let value = unsafe { assign.get_lit_value_unchecked(lit) };
            match value {
                BoolValue::Unassigned | BoolValue::Assigned(true) => {
                    // Check if this literal is not already watched.
                    if !self.watches.contains(&lit) {
                        self.last_pos = ((self.last_pos + n) % card.len()) + 1;
                        self.watches.push(lit);
                        return WatchUpdate::UnknownSingle {
                            propagated: None,
                            new_watches: lit,
                        };
                    }
                }
                BoolValue::Assigned(false) => {}
            }
        }

        // Unable to find a new literal to watch, hence we add the old watch back again. Maybe other watched literals are already satisfied.
        for &lit in self.watches.iter() {
            let value = unsafe { assign.get_lit_value_unchecked(lit) };
            if value == BoolValue::Assigned(false) {
                self.watches.push(old_watch);
                return WatchUpdate::Conflict;
            }
        }

        // Cardinality constraint is propagating and will not propagate again. Hence, we can directly reset it by continuing to watch the `old_watch`.
        self.watches.push(old_watch);
        WatchUpdate::Unknown {
            propagated: self.watches[..self.watches.len() - 1].to_vec(),
            new_watches: vec![old_watch],
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
