use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;

use crate::output::{FileOverview, ModuleOverview, PackageOverview, build_file_overview};
use crate::state::ServerState;
use go_model::TopLevelDecl;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DescribeFileInput {
    pub path: PathBuf,
    #[serde(default)]
    pub include_docs: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DescribeModuleInput {
    pub depth: Option<usize>,
    #[serde(default)]
    pub include_docs: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum DescribeError {
    #[error("state error: {0}")]
    State(String),
    #[error("file not found at {path}: {message}")]
    FileNotFound { path: PathBuf, message: String },
    #[error("failed to parse {path}: {message}")]
    ParseFailed { path: PathBuf, message: String },
}

pub fn handle_describe_file(
    state: &mut ServerState,
    input: DescribeFileInput,
) -> Result<FileOverview, DescribeError> {
    let repo = state
        .repo()
        .map_err(|e| DescribeError::State(e.to_string()))?;
    let root = repo.root().to_owned();

    let raw_path = if input.path.is_absolute() {
        input.path.clone()
    } else {
        root.join(&input.path)
    };

    let target = raw_path
        .canonicalize()
        .map_err(|e| DescribeError::FileNotFound {
            path: raw_path.clone(),
            message: e.to_string(),
        })?;

    let source = std::fs::read(&target).map_err(|e| DescribeError::FileNotFound {
        path: target.clone(),
        message: e.to_string(),
    })?;

    let ast =
        go_analyzer::walker::parse_and_walk(&source).map_err(|e| DescribeError::ParseFailed {
            path: target.clone(),
            message: e.to_string(),
        })?;

    Ok(build_file_overview(&ast, &target, input.include_docs))
}

pub fn handle_describe_module(
    state: &mut ServerState,
    input: DescribeModuleInput,
) -> Result<ModuleOverview, DescribeError> {
    let repo = state
        .repo()
        .map_err(|e| DescribeError::State(e.to_string()))?;
    let root = repo.root().to_owned();

    let module_name = read_module_name(&root);

    let mut packages = Vec::new();
    scan_packages(&root, &root, &module_name, 0, input.depth, &mut packages);
    packages.sort_by(|a, b| a.import_path.cmp(&b.import_path));

    Ok(ModuleOverview {
        module: module_name,
        path: root,
        packages,
    })
}

/// Read the module name from go.mod in the given directory, falling back to the directory name.
fn read_module_name(root: &std::path::Path) -> String {
    let go_mod = root.join("go.mod");
    if let Ok(contents) = std::fs::read_to_string(&go_mod) {
        for line in contents.lines() {
            let line = line.trim();
            if let Some(name) = line.strip_prefix("module ") {
                return name.trim().to_owned();
            }
        }
    }
    // Fall back to the directory name.
    root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".to_owned())
}

/// Recursively scan directories for Go packages, respecting the optional depth limit.
fn scan_packages(
    dir: &std::path::Path,
    root: &std::path::Path,
    module_name: &str,
    current_depth: usize,
    max_depth: Option<usize>,
    out: &mut Vec<PackageOverview>,
) {
    // Collect .go files in this directory.
    let go_files: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "go"))
            .collect(),
        Err(_) => return,
    };

    if !go_files.is_empty() {
        let pkg = build_package_overview(dir, root, module_name, &go_files);
        out.push(pkg);
    }

    // Respect depth limit: if we've hit it, don't recurse further.
    if let Some(max) = max_depth
        && current_depth >= max
    {
        return;
    }

    // Recurse into subdirectories, skipping hidden dirs, vendor, and node_modules.
    let subdirs: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_ok_and(|ft| ft.is_dir())
                    && !e.file_name().to_string_lossy().starts_with('.')
                    && e.file_name() != "vendor"
                    && e.file_name() != "node_modules"
            })
            .map(|e| e.path())
            .collect(),
        Err(_) => return,
    };

    for subdir in subdirs {
        scan_packages(
            &subdir,
            root,
            module_name,
            current_depth + 1,
            max_depth,
            out,
        );
    }
}

/// Build a `PackageOverview` for a directory with Go files.
fn build_package_overview(
    dir: &std::path::Path,
    root: &std::path::Path,
    module_name: &str,
    go_files: &[PathBuf],
) -> PackageOverview {
    let rel = dir.strip_prefix(root).unwrap_or(dir);

    let import_path = if rel == std::path::Path::new("") {
        module_name.to_owned()
    } else {
        format!("{}/{}", module_name, rel.to_string_lossy())
    };

    let mut pkg_name = String::new();
    let mut types = 0usize;
    let mut functions = 0usize;
    let mut methods = 0usize;
    let mut constants = 0usize;
    let mut file_names = Vec::new();

    for file_path in go_files {
        let Ok(source) = std::fs::read(file_path) else {
            continue;
        };
        let Ok(ast) = go_analyzer::walker::parse_and_walk(&source) else {
            continue;
        };

        if pkg_name.is_empty() {
            pkg_name = ast.package.name.clone();
        }

        let name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        file_names.push(name);

        for decl in &ast.decls {
            match decl {
                TopLevelDecl::Func(_) => functions += 1,
                TopLevelDecl::Method(_) => methods += 1,
                TopLevelDecl::Type(specs) => types += specs.len(),
                TopLevelDecl::Const(specs) => {
                    for spec in specs {
                        constants += spec.names.len();
                    }
                }
                TopLevelDecl::Var(_) => {}
            }
        }
    }

    file_names.sort();

    PackageOverview {
        name: pkg_name,
        import_path,
        path: dir.to_owned(),
        files: file_names,
        types,
        functions,
        methods,
        constants,
        doc: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ServerState;

    fn fixture_state() -> ServerState {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../go-analyzer/tests/fixture_repo");
        ServerState::new(path.canonicalize().unwrap())
    }

    #[test]
    fn test_describe_file_has_types_and_functions() {
        let mut state = fixture_state();
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../go-analyzer/tests/fixture_repo/alpha/models.go")
            .canonicalize()
            .unwrap();

        let overview = handle_describe_file(
            &mut state,
            DescribeFileInput {
                path: fixture_path,
                include_docs: false,
            },
        )
        .unwrap();

        assert_eq!(overview.package, "alpha");
        let type_names: Vec<&str> = overview.types.iter().map(|t| t.name.as_str()).collect();
        assert!(
            type_names.contains(&"User"),
            "expected User, got: {type_names:?}"
        );
        assert!(
            type_names.contains(&"Admin"),
            "expected Admin, got: {type_names:?}"
        );
        assert!(
            type_names.contains(&"Config"),
            "expected Config, got: {type_names:?}"
        );
        assert!(
            !overview.functions.is_empty(),
            "expected non-empty functions"
        );
        assert!(!overview.methods.is_empty(), "expected non-empty methods");
    }

    #[test]
    fn test_describe_file_includes_imports() {
        let mut state = fixture_state();
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../go-analyzer/tests/fixture_repo/alpha/models.go")
            .canonicalize()
            .unwrap();

        let overview = handle_describe_file(
            &mut state,
            DescribeFileInput {
                path: fixture_path,
                include_docs: false,
            },
        )
        .unwrap();

        assert!(
            overview.imports.contains(&"fmt".to_owned()),
            "expected 'fmt' in imports, got: {:?}",
            overview.imports
        );
    }

    #[test]
    fn test_describe_module_lists_packages() {
        let mut state = fixture_state();

        let overview = handle_describe_module(
            &mut state,
            DescribeModuleInput {
                depth: None,
                include_docs: false,
            },
        )
        .unwrap();

        let import_paths: Vec<&str> = overview
            .packages
            .iter()
            .map(|p| p.import_path.as_str())
            .collect();
        let has_alpha = import_paths.iter().any(|p| p.contains("alpha"));
        let has_beta = import_paths.iter().any(|p| p.contains("beta"));
        assert!(has_alpha, "expected alpha package, got: {import_paths:?}");
        assert!(has_beta, "expected beta package, got: {import_paths:?}");
    }

    #[test]
    fn test_describe_module_has_counts() {
        let mut state = fixture_state();

        let overview = handle_describe_module(
            &mut state,
            DescribeModuleInput {
                depth: None,
                include_docs: false,
            },
        )
        .unwrap();

        let alpha = overview
            .packages
            .iter()
            .find(|p| p.import_path.contains("alpha"))
            .expect("alpha package not found");

        assert!(alpha.types > 0, "expected types > 0 in alpha");
        assert!(alpha.functions > 0, "expected functions > 0 in alpha");
    }

    #[test]
    fn test_describe_module_respects_depth() {
        let mut state = fixture_state();

        let shallow = handle_describe_module(
            &mut state,
            DescribeModuleInput {
                depth: Some(0),
                include_docs: false,
            },
        )
        .unwrap();

        let deep = handle_describe_module(
            &mut state,
            DescribeModuleInput {
                depth: None,
                include_docs: false,
            },
        )
        .unwrap();

        assert!(
            shallow.packages.len() <= deep.packages.len(),
            "depth 0 should have <= packages compared to depth None: {} vs {}",
            shallow.packages.len(),
            deep.packages.len()
        );
    }
}
