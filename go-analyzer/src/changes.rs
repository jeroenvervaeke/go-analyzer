use crate::edit::Edit;

/// A set of pending edits to source files, produced by selection operations.
///
/// `Changes` is pure data -- no I/O happens until [`Repo::apply`](crate::Repo::apply)
/// is called.
///
/// # Example
///
/// ```no_run
/// # use go_analyzer::*;
/// # use go_model::*;
/// let repo = Repo::load(".")?;
/// let c1 = repo.functions().named("oldHelper").delete();
/// let c2 = repo.functions().named("OldAPI").rename("NewAPI");
/// let combined = Changes::combine([c1, c2]);
/// repo.apply(combined).commit()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct Changes {
    pub(crate) edits: Vec<Edit>,
}

impl Changes {
    /// Create an empty changeset with no edits.
    pub fn none() -> Self {
        Self { edits: vec![] }
    }

    /// Create changes from a raw list of edits.
    pub fn from_edits(edits: Vec<Edit>) -> Self {
        Self { edits }
    }

    /// Merge multiple changesets into one.
    pub fn combine(iter: impl IntoIterator<Item = Changes>) -> Self {
        let mut edits = Vec::new();
        for c in iter {
            edits.extend(c.edits);
        }
        Self { edits }
    }

    /// Combine `self` with `other` into a single changeset.
    pub fn and(self, other: Changes) -> Self {
        Self::combine([self, other])
    }

    /// Return `true` if there are no pending edits.
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Return the number of individual edits in this changeset.
    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }
}
