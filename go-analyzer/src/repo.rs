use std::collections::HashMap;
use std::path::{Path, PathBuf};

use go_model::{FuncDecl, MethodDecl, SourceFile, TopLevelDecl, TypeSpec};

use crate::applied::Applied;
use crate::changes::Changes;
use crate::edit::apply_edits;
use crate::selection::{Selection, SelectionItem};

/// A loaded Go repository: parsed AST for every `.go` file under a root directory.
///
/// `Repo` is the main entry point for all analysis and transformation.
///
/// # Example
///
/// ```no_run
/// # use go_analyzer::*;
/// let repo = Repo::load(".")?;
/// let n = repo.structs().exported().count();
/// println!("{n} exported structs");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct Repo {
    pub(crate) _root: PathBuf,
    pub(crate) files: HashMap<PathBuf, RepoFile>,
}

pub(crate) struct RepoFile {
    pub source: Vec<u8>,
    pub ast: SourceFile,
}

impl Repo {
    /// Recursively load all `.go` files under `path`, parse each one, and
    /// skip any that fail to parse.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let root = path.as_ref().canonicalize()?;
        let mut files = HashMap::new();

        for entry in walkdir(&root)? {
            let Ok(source) = std::fs::read(&entry) else {
                continue;
            };
            let Ok(ast) = crate::walker::parse_and_walk(&source) else {
                continue;
            };
            files.insert(entry, RepoFile { source, ast });
        }

        Ok(Self { _root: root, files })
    }

    /// Return the number of successfully parsed `.go` files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Return the root directory this repo was loaded from.
    pub fn root(&self) -> &Path {
        &self._root
    }

    /// Return the package name for a file path, or None if the file isn't in the repo.
    pub fn package_for_file(&self, file: &Path) -> Option<&str> {
        self.files.get(file).map(|rf| rf.ast.package.name.as_str())
    }

    /// Select all top-level function declarations across the repository.
    pub fn functions(&self) -> Selection<'_, FuncDecl> {
        let items = self
            .files
            .iter()
            .flat_map(|(path, rf)| {
                rf.ast.decls.iter().filter_map(move |d| match d {
                    TopLevelDecl::Func(f) => Some(SelectionItem {
                        item: (**f).clone(),
                        file: path.clone(),
                    }),
                    _ => None,
                })
            })
            .collect();
        Selection { repo: self, items }
    }

    /// Select all method declarations across the repository.
    pub fn methods(&self) -> Selection<'_, MethodDecl> {
        let items = self
            .files
            .iter()
            .flat_map(|(path, rf)| {
                rf.ast.decls.iter().filter_map(move |d| match d {
                    TopLevelDecl::Method(m) => Some(SelectionItem {
                        item: (**m).clone(),
                        file: path.clone(),
                    }),
                    _ => None,
                })
            })
            .collect();
        Selection { repo: self, items }
    }

    /// Select all type declarations (structs, interfaces, aliases, etc.) across the repository.
    pub fn types(&self) -> Selection<'_, TypeSpec> {
        let items = self
            .files
            .iter()
            .flat_map(|(path, rf)| {
                rf.ast.decls.iter().flat_map(move |d| match d {
                    TopLevelDecl::Type(specs) => specs
                        .iter()
                        .map(|t| SelectionItem {
                            item: t.clone(),
                            file: path.clone(),
                        })
                        .collect::<Vec<_>>(),
                    _ => vec![],
                })
            })
            .collect();
        Selection { repo: self, items }
    }

    /// Select all struct type declarations. Shorthand for `types().structs()`.
    pub fn structs(&self) -> Selection<'_, TypeSpec> {
        self.types().structs()
    }

    /// Select all interface type declarations. Shorthand for `types().interfaces()`.
    pub fn interfaces(&self) -> Selection<'_, TypeSpec> {
        self.types().interfaces()
    }

    /// Apply changes to the repo, producing an `Applied` that can be previewed
    /// or committed.
    pub fn apply(&self, changes: Changes) -> Applied<'_> {
        // Group edits by file path.
        let mut per_file: HashMap<PathBuf, Vec<crate::edit::Edit>> = HashMap::new();
        for edit in changes.edits {
            per_file.entry(edit.file.clone()).or_default().push(edit);
        }

        // Apply edits per file, tracking only successfully applied edits.
        let mut results = HashMap::new();
        let mut applied_edit_count = 0usize;
        for (path, edits) in per_file {
            let Some(rf) = self.files.get(&path) else {
                continue;
            };
            let file_edit_count = edits.len();
            match apply_edits(&rf.source, &edits) {
                Ok(modified) => {
                    results.insert(path, modified);
                    applied_edit_count += file_edit_count;
                }
                Err(err) => {
                    eprintln!(
                        "warning: failed to apply edits to {}: {err}",
                        path.display()
                    );
                }
            }
        }

        Applied {
            repo: self,
            results,
            edit_count: applied_edit_count,
        }
    }
}

/// Recursively collect all `.go` file paths under `root`.
fn walkdir(root: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut result = Vec::new();
    walk_recursive(root, &mut result)?;
    result.sort();
    Ok(result)
}

fn walk_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            // Skip hidden dirs and common non-Go dirs.
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') || name_str == "vendor" || name_str == "node_modules" {
                continue;
            }
            walk_recursive(&path, out)?;
        } else if ft.is_file() && path.extension().is_some_and(|ext| ext == "go") {
            out.push(path);
        }
    }
    Ok(())
}
