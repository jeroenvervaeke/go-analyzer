use std::path::PathBuf;

use go_model::Span;

#[derive(Debug, Clone)]
pub struct Edit {
    pub file: PathBuf,
    pub kind: EditKind,
}

#[derive(Debug, Clone)]
pub enum EditKind {
    Replace {
        span: Span,
        new_text: String,
    },
    Delete {
        span: Span,
    },
    InsertAfter {
        anchor_byte: usize,
        new_text: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("overlapping edits at bytes {a_start}..{a_end} and {b_start}..{b_end}")]
    Overlapping {
        a_start: usize,
        a_end: usize,
        b_start: usize,
        b_end: usize,
    },
    #[error("edit span {start}..{end} out of bounds for source of length {source_len}")]
    OutOfBounds {
        start: usize,
        end: usize,
        source_len: usize,
    },
}

/// Represents a resolved byte-range operation on a single file's source.
struct ResolvedEdit {
    start: usize,
    end: usize,
    replacement: Option<String>,
}

/// Apply a set of edits (all for the same file) to the given source bytes.
///
/// Edits are sorted by start byte, checked for overlaps, and applied in
/// reverse order so that earlier byte offsets remain valid.
pub fn apply_edits(source: &[u8], edits: &[Edit]) -> Result<Vec<u8>, ApplyError> {
    let mut resolved: Vec<ResolvedEdit> = edits
        .iter()
        .map(|e| match &e.kind {
            EditKind::Replace { span, new_text } => ResolvedEdit {
                start: span.start_byte,
                end: span.end_byte,
                replacement: Some(new_text.clone()),
            },
            EditKind::Delete { span } => ResolvedEdit {
                start: span.start_byte,
                end: span.end_byte,
                replacement: None,
            },
            EditKind::InsertAfter {
                anchor_byte,
                new_text,
            } => ResolvedEdit {
                start: *anchor_byte,
                end: *anchor_byte,
                replacement: Some(new_text.clone()),
            },
        })
        .collect();

    // Bounds check: reject edits that extend past the source or have inverted spans.
    let source_len = source.len();
    for edit in &resolved {
        if edit.end > source_len || edit.start > source_len || edit.start > edit.end {
            return Err(ApplyError::OutOfBounds {
                start: edit.start,
                end: edit.end,
                source_len,
            });
        }
    }

    // Sort by start byte, ties broken by end byte descending (wider spans first
    // so overlap detection catches them).
    resolved.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));

    // Check for overlapping edits (insertions at the same point are fine).
    for pair in resolved.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        // Two zero-width insertions at the same point are allowed.
        if a.start == a.end && b.start == b.end {
            continue;
        }
        if a.end > b.start {
            return Err(ApplyError::Overlapping {
                a_start: a.start,
                a_end: a.end,
                b_start: b.start,
                b_end: b.end,
            });
        }
    }

    // Apply in reverse order so byte offsets stay valid.
    let mut result = source.to_vec();
    for edit in resolved.iter().rev() {
        match &edit.replacement {
            Some(text) => {
                result.splice(edit.start..edit.end, text.bytes());
            }
            None => {
                // Delete with whitespace cleanup.
                let (start, end) = cleaned_delete_range(&result, edit.start, edit.end);
                result.splice(start..end, std::iter::empty());
            }
        }
    }

    Ok(result)
}

/// Expand a delete range to clean up surrounding whitespace.
///
/// Strategy: if the deleted region is on its own line(s) — i.e. there is only
/// whitespace between the line start and the span start, and only whitespace
/// (plus a newline) between the span end and the next newline — expand the
/// deletion to include the entire line(s) plus the trailing newline so no
/// blank line remains.
fn cleaned_delete_range(source: &[u8], start: usize, end: usize) -> (usize, usize) {
    // Find the start of the line containing `start`.
    let line_start = source[..start]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |pos| pos + 1);

    // Check if everything between line_start and start is whitespace.
    let prefix_is_blank = source[line_start..start]
        .iter()
        .all(|b| b.is_ascii_whitespace() && *b != b'\n');

    // Find the end of the line containing `end`.
    let line_end = source[end..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(source.len(), |pos| end + pos + 1);

    // Check if everything between end and line_end is whitespace (before the newline).
    let suffix_is_blank = source[end..line_end]
        .iter()
        .all(|b| b.is_ascii_whitespace());

    if prefix_is_blank && suffix_is_blank {
        // Also consume a preceding blank line if the line before is empty.
        let mut expanded_start = line_start;
        if expanded_start > 0 && source[expanded_start - 1] == b'\n' {
            let prev_line_start = source[..expanded_start - 1]
                .iter()
                .rposition(|&b| b == b'\n')
                .map_or(0, |pos| pos + 1);
            let prev_line = &source[prev_line_start..expanded_start - 1];
            if prev_line.iter().all(|b| b.is_ascii_whitespace()) {
                expanded_start = prev_line_start;
            }
        }
        (expanded_start, line_end)
    } else {
        (start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_edit(kind: EditKind) -> Edit {
        Edit {
            file: PathBuf::from("test.go"),
            kind,
        }
    }

    fn delete_span(start: usize, end: usize) -> Edit {
        let span = Span {
            start_byte: start,
            end_byte: end,
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 0,
        };
        make_edit(EditKind::Delete { span })
    }

    fn replace_span(start: usize, end: usize, text: &str) -> Edit {
        let span = Span {
            start_byte: start,
            end_byte: end,
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 0,
        };
        make_edit(EditKind::Replace {
            span,
            new_text: text.to_owned(),
        })
    }

    fn insert_after(anchor: usize, text: &str) -> Edit {
        make_edit(EditKind::InsertAfter {
            anchor_byte: anchor,
            new_text: text.to_owned(),
        })
    }

    #[test]
    fn test_replace_single_span() {
        let source = b"func Foo() {}";
        let edits = vec![replace_span(5, 8, "Bar")];
        let result = apply_edits(source, &edits).expect("should apply");
        assert_eq!(String::from_utf8(result).unwrap(), "func Bar() {}");
    }

    #[test]
    fn test_delete_whole_line() {
        let source = b"line1\nline2\nline3\n";
        let edits = vec![delete_span(6, 11)]; // "line2"
        let result = apply_edits(source, &edits).expect("should apply");
        assert_eq!(String::from_utf8(result).unwrap(), "line1\nline3\n");
    }

    #[test]
    fn test_insert_after() {
        let source = b"abc";
        let edits = vec![insert_after(3, "def")];
        let result = apply_edits(source, &edits).expect("should apply");
        assert_eq!(String::from_utf8(result).unwrap(), "abcdef");
    }

    #[test]
    fn test_overlapping_edits_rejected() {
        let source = b"abcdef";
        let edits = vec![replace_span(1, 4, "X"), replace_span(3, 6, "Y")];
        let result = apply_edits(source, &edits);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_non_overlapping_edits() {
        let source = b"func Foo() {}\nfunc Bar() {}";
        let edits = vec![replace_span(5, 8, "AAA"), replace_span(19, 22, "BBB")];
        let result = apply_edits(source, &edits).expect("should apply");
        assert_eq!(
            String::from_utf8(result).unwrap(),
            "func AAA() {}\nfunc BBB() {}"
        );
    }
}
