use serde::{Deserialize, Serialize};

/// Represents a source location range within a Go file.
///
/// Use [`Span::synthetic()`] for generated nodes that have no real source location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

impl Span {
    /// Sentinel value for generated nodes that have no source location.
    /// The printer ignores spans. The edit engine uses this to distinguish
    /// "replace existing source" (real span) from "insert new source" (synthetic).
    pub fn synthetic() -> Self {
        Self {
            start_byte: 0,
            end_byte: 0,
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 0,
        }
    }

    /// Returns `true` if this span was created by [`Span::synthetic()`].
    pub fn is_synthetic(&self) -> bool {
        self.start_byte == 0 && self.end_byte == 0
    }
}
