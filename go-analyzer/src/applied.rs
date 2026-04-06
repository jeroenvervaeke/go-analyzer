use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::repo::Repo;

/// The result of applying [`Changes`](crate::Changes) to a [`Repo`].
///
/// Holds the modified source in memory. Use [`preview`](Self::preview) to print
/// a unified diff, [`dry_run`](Self::dry_run) to inspect changes, or
/// [`commit`](Self::commit) to write files to disk.
///
/// # Example
///
/// ```no_run
/// # use go_analyzer::*;
/// # use go_model::*;
/// let repo = Repo::load(".")?;
/// let changes = repo.functions().named("Deprecated").delete();
/// let summary = repo.apply(changes).preview().commit()?;
/// println!("modified {} files", summary.files_modified);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct Applied<'repo> {
    pub(crate) repo: &'repo Repo,
    pub(crate) results: HashMap<PathBuf, Vec<u8>>,
    pub(crate) edit_count: usize,
}

/// Summary returned by [`Applied::commit`] after writing changes to disk.
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

    /// Return the total number of individual edits that were applied.
    pub fn edit_count(&self) -> usize {
        self.edit_count
    }

    /// Write modified files to disk.
    pub fn commit(self) -> Result<CommitSummary, Box<dyn std::error::Error>> {
        let files_modified = self.results.len();
        let edits_applied = self.edit_count;

        for (path, new_bytes) in &self.results {
            std::fs::write(path, new_bytes)?;
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
