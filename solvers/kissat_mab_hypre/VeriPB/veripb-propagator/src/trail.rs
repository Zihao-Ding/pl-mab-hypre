use core::panic;
use std::rc::Rc;

use veripb_formula::prelude::*;

use crate::error::PropagatorError;

#[derive(Debug, Default, Clone, PartialEq)]
pub enum Reason {
    #[default]
    Assumption,
    Constraint(Rc<DBConstraint>),
}

impl Reason {
    /// Get the reason constraint from the `Reason`.
    ///
    /// This function panics if the `Reason` was an `Assumption`.
    #[inline]
    pub fn unwrap(&self) -> &Rc<DBConstraint> {
        match self {
            Reason::Assumption => panic!("The reason is not a constraint."),
            Reason::Constraint(constraint) => constraint,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Propagation {
    pub lit: Lit,
    pub reason: Reason,
}

impl Propagation {
    pub fn new(lit: Lit, reason: Reason) -> Self {
        Propagation { lit, reason }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Trail {
    pub assignment: Assignment<BooleanVar>,
    pub trail: Vec<Propagation>,
    pub conflict: Option<Reason>,
}

impl Trail {
    #[inline]
    pub fn with_size(size: usize) -> Self {
        Trail {
            assignment: Assignment::with_size(size),
            ..Default::default()
        }
    }

    #[inline]
    pub fn push(
        &mut self,
        propagation: Propagation,
        mark_reason: bool,
    ) -> Result<(), PropagatorError> {
        match unsafe { self.assignment.get_lit_value_unchecked(propagation.lit) } {
            BoolValue::Unassigned => {
                unsafe {
                    self.assignment
                        .set_lit_value_unchecked(propagation.lit, BoolValue::Assigned(true))
                };
                if mark_reason {
                    if let Reason::Constraint(constraint) = &propagation.reason {
                        constraint.header.borrow_mut().is_saved_reason = true;
                    }
                }
                self.trail.push(propagation);
            }
            BoolValue::Assigned(false) => {
                if mark_reason {
                    if let Reason::Constraint(constraint) = &propagation.reason {
                        constraint.header.borrow_mut().is_saved_reason = true;
                    }
                }
                self.conflict = Some(propagation.reason);
                return Err(PropagatorError::TrailContainsNegatedLit);
            }
            BoolValue::Assigned(true) => {}
        }

        Ok(())
    }

    #[inline]
    pub fn pop(&mut self) -> Option<Propagation> {
        match self.trail.pop() {
            Some(propagation) => {
                unsafe {
                    self.assignment
                        .set_lit_value_unchecked(propagation.lit, BoolValue::Unassigned)
                };
                Some(propagation)
            }
            None => None,
        }
    }

    /// Number of variables the assignment in the trail can keep a track of.
    #[inline]
    pub fn size(&self) -> usize {
        self.assignment.len()
    }

    #[inline]
    pub fn resize(&mut self, new_len: usize) {
        if new_len != self.size() {
            self.assignment.resize(new_len);
        }
    }

    /// Check if the trail is conflicting.
    #[inline]
    pub fn is_conflicting(&self) -> bool {
        self.conflict.is_some()
    }

    /// The length of the trail, which is equivalent to the number of assigned variables by the trail.
    #[inline]
    pub fn len(&self) -> usize {
        self.trail.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn add_assumptions(&mut self, assumptions: &Vec<Lit>) -> Result<(), PropagatorError> {
        for lit in assumptions {
            self.push(Propagation::new(*lit, Reason::Assumption), false)?
        }
        Ok(())
    }
}

impl From<(&Vec<Lit>, Assignment<BooleanVar>)> for Trail {
    fn from(value: (&Vec<Lit>, Assignment<BooleanVar>)) -> Self {
        let (lierals, assignment) = value;
        let mut trail = Trail {
            assignment,
            ..Default::default()
        };
        for lit in lierals {
            trail.trail.push(Propagation::new(*lit, Reason::Assumption));
        }

        trail
    }
}
