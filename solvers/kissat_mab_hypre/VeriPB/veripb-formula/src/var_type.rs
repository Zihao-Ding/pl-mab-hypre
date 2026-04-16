//! Definition related to variables.

use crate::prelude::ToPrettyString;

/// Type definition for a variable index. The variable index is just `usize`, but the separate name makes it easier to identify what a variable is.
pub type VarIdx = usize;

impl ToPrettyString for VarIdx {
    fn to_pretty_string(&self, var_names: &crate::prelude::VarNameManager) -> String {
        var_names.get_name(*self).to_string()
    }
}

/// Generic variable trait to represent a variable with potentially arbitrary names.
///
/// High-level properties of a variable:
/// - is either unassigned (`None`) or assigned (`Some(v)`, where `v` could be something like True, False, integer, float, ...)
///
pub trait VarType: Clone + Default {
    /// The type deteriming the value of the [`VarType`].
    type Value: Negate + Default;
    /// Create a new variable.
    fn new() -> Self
    where
        Self: Sized;

    /// Set value of the variable.
    fn set_value(&mut self, value: Self::Value);

    /// Get value of the variable.
    fn get_value(&self) -> Self::Value;
}

/// Trait for in situ negation of a [`VarType::Value`].
pub trait Negate {
    /// Get the negation of the value.
    fn negate(self) -> Self;
}
