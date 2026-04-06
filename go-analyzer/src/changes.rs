use crate::edit::Edit;

pub struct Changes {
    pub(crate) edits: Vec<Edit>,
}

impl Changes {
    pub fn none() -> Self {
        Self { edits: vec![] }
    }

    /// Create changes from a raw list of edits.
    pub fn from_edits(edits: Vec<Edit>) -> Self {
        Self { edits }
    }

    pub fn combine(iter: impl IntoIterator<Item = Changes>) -> Self {
        let mut edits = Vec::new();
        for c in iter {
            edits.extend(c.edits);
        }
        Self { edits }
    }

    pub fn and(self, other: Changes) -> Self {
        Self::combine([self, other])
    }

    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }
}
