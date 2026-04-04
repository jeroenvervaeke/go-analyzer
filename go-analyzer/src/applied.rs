use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::printer::Printer;
use crate::repo::Repo;

pub struct Applied<'repo> {
    pub(crate) repo: &'repo Repo,
    pub(crate) results: HashMap<PathBuf, Vec<u8>>,
    pub(crate) edit_count: usize,
}

pub struct CommitSummary {
    pub files_modified: usize,
    pub edits_applied: usize,
}

impl<'repo> Applied<'repo> {
    /// Print a unified diff of all changes to stdout and return `self` for
    /// chaining.
    pub fn preview(self) -> Self {
        for (path, new_bytes) in &self.results {
            let Some(rf) = self.repo.files.get(path) else {
                continue;
            };
            let old = String::from_utf8_lossy(&rf.source);
            let new = String::from_utf8_lossy(new_bytes);
            let diff = unified_diff(path, &old, &new);
            if !diff.is_empty() {
                print!("{diff}");
            }
        }
        self
    }

    /// Return the modified source as UTF-8 strings, keyed by path.
    pub fn dry_run(&self) -> HashMap<PathBuf, String> {
        self.results
            .iter()
            .map(|(path, bytes)| (path.clone(), String::from_utf8_lossy(bytes).into_owned()))
            .collect()
    }

    /// Return the paths of all files that were modified.
    pub fn affected_files(&self) -> Vec<&Path> {
        self.results.keys().map(|p| p.as_path()).collect()
    }

    pub fn edit_count(&self) -> usize {
        self.edit_count
    }

    /// Write modified files to disk and run `gofmt` on each.
    pub fn commit(self) -> Result<CommitSummary, Box<dyn std::error::Error>> {
        let files_modified = self.results.len();
        let edits_applied = self.edit_count;

        for (path, new_bytes) in &self.results {
            // Write the raw modified source first.
            std::fs::write(path, new_bytes)?;

            // Run gofmt to normalize formatting.
            let source_str = String::from_utf8_lossy(new_bytes);
            let formatted = Printer::gofmt(&source_str);
            std::fs::write(path, formatted.as_bytes())?;
        }

        Ok(CommitSummary {
            files_modified,
            edits_applied,
        })
    }
}

/// Produce a unified diff string between old and new content for a given path.
fn unified_diff(path: &Path, old: &str, new: &str) -> String {
    use similar::{ChangeTag, TextDiff};

    let diff = TextDiff::from_lines(old, new);
    let mut output = String::new();
    let path_str = path.display().to_string();

    let has_changes = diff.iter_all_changes().any(|c| c.tag() != ChangeTag::Equal);

    if !has_changes {
        return output;
    }

    output.push_str(&format!("--- a/{path_str}\n"));
    output.push_str(&format!("+++ b/{path_str}\n"));

    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        output.push_str(&hunk.to_string());
    }

    output
}
