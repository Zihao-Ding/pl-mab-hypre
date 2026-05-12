use crate::{context::Context, database::Database, error::CheckingError, rules::Deletion};
use colored::Colorize;

pub trait DeletionSequence {
    /// Get an [`Iterator`] of the constraint IDs that are deleted.
    fn get_deleted_ids(&self) -> impl Iterator<Item = &usize>;

    /// Write the deleted constraint IDs to a [`String`].
    #[inline]
    fn trace_deletions(&self) -> Option<String> {
        self.get_deleted_ids()
            .map(|id| id.to_string().bright_green().to_string())
            .reduce(|acc, value| acc + ", " + value.as_str())
    }

    /// Delete the constraint IDs from the [`Database`].
    #[inline]
    fn delete_constraints(
        &self,
        database: &mut Database,
        context: &mut Context,
    ) -> Result<(), CheckingError> {
        self.get_deleted_ids()
            .try_for_each(|&constraint_id| database.delete_constraint(context, constraint_id))
    }
}

impl DeletionSequence for Vec<usize> {
    #[inline]
    fn get_deleted_ids(&self) -> impl Iterator<Item = &usize> {
        self.iter()
    }
}

pub enum DeletionSequenceEnum<'a> {
    Deletion(&'a Deletion),
    Vec(Vec<usize>),
}

impl DeletionSequenceEnum<'_> {
    #[inline]
    pub fn trace_deletions(&self) -> Option<String> {
        match self {
            DeletionSequenceEnum::Deletion(del) => del.trace_deletions(),
            DeletionSequenceEnum::Vec(vec) => vec.trace_deletions(),
        }
    }

    #[inline]
    pub fn delete_constraints(
        &self,
        database: &mut Database,
        context: &mut Context,
    ) -> Result<(), CheckingError> {
        match self {
            DeletionSequenceEnum::Deletion(del) => del.delete_constraints(database, context),
            DeletionSequenceEnum::Vec(vec) => vec.delete_constraints(database, context),
        }
    }
}
