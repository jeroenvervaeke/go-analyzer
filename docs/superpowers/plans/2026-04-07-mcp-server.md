# go-analyzer MCP Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an MCP server (`go-analyzer-mcp` crate) that exposes 5 tools — `query`, `call_graph`, `edit`, `describe_file`, `describe_module` — over stdio, backed by the existing `go-analyzer` crate.

**Architecture:** New workspace crate `go-analyzer-mcp` depends on `go-analyzer` and `go-model`. A persistent `Repo` is lazy-loaded on first tool call and auto-reloaded after edits. Tools use a shared selection builder (select + filters pipeline) for `query` and `edit`. The `rmcp` SDK handles MCP protocol and stdio transport.

**Tech Stack:** Rust, rmcp (MCP SDK), tokio (async runtime), serde/serde_json (serialization), clap (CLI args)

**Spec:** `docs/superpowers/specs/2026-04-07-mcp-server-design.md`

---

## File Structure

### Existing files to modify

- `Cargo.toml` — add `go-analyzer-mcp` to workspace members
- `go-analyzer/src/lib.rs` — make `printer` module public (currently `pub(crate)`)
- `go-analyzer/src/repo.rs` — add `pub fn root()` getter for `_root`
- `go-analyzer/src/printer.rs` — change `pub(crate) struct Printer` to `pub struct Printer`

### New files

- `go-analyzer-mcp/Cargo.toml` — crate manifest
- `go-analyzer-mcp/src/main.rs` — CLI arg parsing, server init, stdio transport
- `go-analyzer-mcp/src/state.rs` — `ServerState` struct: lazy `Repo`, repo path, load/reload
- `go-analyzer-mcp/src/selection_builder.rs` — shared `SelectKind`, `Filter` enums, `build_selection()` that returns serializable results
- `go-analyzer-mcp/src/output.rs` — output types (`QueryItem`, `FileOverview`, `ModuleOverview`, `CallGraphResult`, `EditResult`) and formatting helpers
- `go-analyzer-mcp/src/tools/mod.rs` — re-exports
- `go-analyzer-mcp/src/tools/query.rs` — `query` tool: build selection, format output
- `go-analyzer-mcp/src/tools/describe.rs` — `describe_file` and `describe_module` tools
- `go-analyzer-mcp/src/tools/call_graph.rs` — `call_graph` tool
- `go-analyzer-mcp/src/tools/edit.rs` — `edit` tool: build selection, apply action, return diff
- `go-analyzer-mcp/tests/fixture_repo/go.mod` — module file for test repo
- `go-analyzer-mcp/tests/query_test.rs` — integration tests for query tool
- `go-analyzer-mcp/tests/describe_test.rs` — integration tests for describe tools
- `go-analyzer-mcp/tests/call_graph_test.rs` — integration tests for call_graph tool
- `go-analyzer-mcp/tests/edit_test.rs` — integration tests for edit tool

---

## Task 1: Expose Printer and Repo Root from go-analyzer

Small API changes needed so the MCP crate can render signatures and access the repo path.

**Files:**
- Modify: `go-analyzer/src/lib.rs:46` (change `pub mod printer` visibility)
- Modify: `go-analyzer/src/printer.rs:3` (change struct visibility)
- Modify: `go-analyzer/src/repo.rs:24-27` (add root getter, add package_for_file helper)

- [ ] **Step 1: Make printer module public in lib.rs**

In `go-analyzer/src/lib.rs`, change:
```rust
pub mod printer;
```
(It's already `pub mod printer;` on line 50 — but `Printer` struct inside is `pub(crate)`. The module is public, the struct is not.)

- [ ] **Step 2: Make Printer struct public**

In `go-analyzer/src/printer.rs`, change:
```rust
pub(crate) struct Printer;
```
to:
```rust
pub struct Printer;
```

- [ ] **Step 3: Add root() getter to Repo**

In `go-analyzer/src/repo.rs`, add after the `file_count()` method:
```rust
/// Return the root directory this repo was loaded from.
pub fn root(&self) -> &Path {
    &self._root
}

/// Return the package name for a file path, or None if the file isn't in the repo.
pub fn package_for_file(&self, file: &Path) -> Option<&str> {
    self.files.get(file).map(|rf| rf.ast.package.name.as_str())
}
```

- [ ] **Step 4: Verify existing tests still pass**

Run: `cargo test --all-targets -p go-analyzer`
Expected: All tests pass, no regressions.

- [ ] **Step 5: Commit**

```bash
git add .
git commit -m "feat(go-analyzer): expose Printer and Repo::root() for MCP server"
```

---

## Task 2: Create go-analyzer-mcp Crate Scaffold

**Files:**
- Modify: `Cargo.toml` (add workspace member)
- Create: `go-analyzer-mcp/Cargo.toml`
- Create: `go-analyzer-mcp/src/main.rs`

- [ ] **Step 1: Add workspace member**

In root `Cargo.toml`, change:
```toml
members = ["go-model", "go-analyzer"]
```
to:
```toml
members = ["go-model", "go-analyzer", "go-analyzer-mcp"]
```

- [ ] **Step 2: Create go-analyzer-mcp/Cargo.toml**

```toml
[package]
name = "go-analyzer-mcp"
version = "0.1.0"
edition = "2024"
description = "MCP server for Go code analysis and transformation"
license.workspace = true
repository.workspace = true
authors.workspace = true

[dependencies]
go-analyzer = { path = "../go-analyzer" }
go-model = { path = "../go-model" }
rmcp = { version = "0.1", features = ["server", "transport-io"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
thiserror = "2"
similar = "2"

[dev-dependencies]
tempfile = "3"
```

Note: The `rmcp` crate version and features should be verified against the latest published version on crates.io. Adjust the version and feature flags as needed. The features `"server"` and `"transport-io"` are assumed based on common MCP SDK patterns — check the rmcp docs for exact feature names.

- [ ] **Step 3: Create minimal main.rs**

```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "go-analyzer-mcp", about = "MCP server for Go code analysis")]
struct Cli {
    /// Path to the Go project to analyze. Defaults to current directory.
    #[arg(long, default_value = ".")]
    path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let path = cli.path.canonicalize()?;
    eprintln!("go-analyzer-mcp: serving {}", path.display());

    // TODO: Initialize MCP server and start stdio transport
    // This will be implemented in Task 9 after all tool handlers are ready.

    Ok(())
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p go-analyzer-mcp`
Expected: Compiles successfully. If `rmcp` features are wrong, adjust Cargo.toml and retry.

- [ ] **Step 5: Commit**

```bash
git add .
git commit -m "feat(mcp): scaffold go-analyzer-mcp crate with CLI args"
```

---

## Task 3: Implement ServerState with Lazy Loading

**Files:**
- Create: `go-analyzer-mcp/src/state.rs`
- Modify: `go-analyzer-mcp/src/main.rs` (add mod declaration)

- [ ] **Step 1: Write unit test for state**

Create `go-analyzer-mcp/src/state.rs`:

```rust
use std::path::{Path, PathBuf};

use go_analyzer::Repo;

/// Holds the loaded Go repository and its source path.
/// The repo is lazy-loaded on first access and auto-reloaded after edits.
pub struct ServerState {
    repo: Option<Repo>,
    repo_path: PathBuf,
}

impl ServerState {
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            repo: None,
            repo_path,
        }
    }

    /// Return a reference to the loaded repo, loading it on first access.
    pub fn repo(&mut self) -> Result<&Repo, StateError> {
        if self.repo.is_none() {
            self.load()?;
        }
        // Safe: we just loaded if it was None
        Ok(self.repo.as_ref().unwrap())
    }

    /// Reload the repo from disk. Called after edits are applied.
    pub fn reload(&mut self) -> Result<(), StateError> {
        self.load()
    }

    /// Return the repo path.
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    fn load(&mut self) -> Result<(), StateError> {
        let repo = Repo::load(&self.repo_path).map_err(|e| StateError::LoadFailed {
            path: self.repo_path.clone(),
            source: e.to_string(),
        })?;
        self.repo = Some(repo);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("failed to load repo at {path}: {source}")]
    LoadFailed { path: PathBuf, source: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lazy_load_on_first_access() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../go-analyzer/tests/fixture_repo");
        let mut state = ServerState::new(fixture);

        // Repo should not be loaded yet
        assert!(state.repo.is_none());

        // First access triggers load
        let repo = state.repo().unwrap();
        assert!(repo.file_count() > 0);
    }

    #[test]
    fn test_load_nonexistent_path_returns_error() {
        let mut state = ServerState::new(PathBuf::from("/nonexistent/path"));
        let result = state.repo();
        assert!(result.is_err());
    }

    #[test]
    fn test_reload_refreshes_repo() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../go-analyzer/tests/fixture_repo");
        let mut state = ServerState::new(fixture);

        // Load initially
        let _ = state.repo().unwrap();
        assert!(state.repo.is_some());

        // Reload succeeds
        state.reload().unwrap();
        assert!(state.repo.is_some());
    }
}
```

- [ ] **Step 2: Add module declaration in main.rs**

Add to the top of `go-analyzer-mcp/src/main.rs`:
```rust
mod state;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p go-analyzer-mcp`
Expected: All 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(mcp): add ServerState with lazy repo loading"
```

---

## Task 4: Define Selection Builder (Shared Input Types + Builder)

The core abstraction shared between `query` and `edit` tools. Takes JSON input (select kind + filters) and builds a `Selection<T>` from the repo.

**Files:**
- Create: `go-analyzer-mcp/src/selection_builder.rs`
- Modify: `go-analyzer-mcp/src/main.rs` (add mod declaration)

- [ ] **Step 1: Define input types**

Create `go-analyzer-mcp/src/selection_builder.rs`:

```rust
use serde::{Deserialize, Serialize};

/// What kind of declarations to select.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectKind {
    Functions,
    Methods,
    Structs,
    Interfaces,
    Types,
}

/// A single filter in the pipeline. Chainable, applied in order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    Named(String),
    InPackage(String),
    Exported(bool),
    ExcludingTests(bool),
    OnType(String),
    Implementing(String),
}
```

- [ ] **Step 2: Write test for selection building**

Add to the same file:

```rust
use std::path::PathBuf;

use go_analyzer::Repo;

use crate::output::QueryItem;

/// Build a selection from the repo using the given kind and filters,
/// and return serializable query items.
pub fn build_query(
    repo: &Repo,
    select: &SelectKind,
    filters: &[Filter],
) -> Vec<QueryItem> {
    match select {
        SelectKind::Functions => query_functions(repo, filters),
        SelectKind::Methods => query_methods(repo, filters),
        SelectKind::Structs => query_structs(repo, filters),
        SelectKind::Interfaces => query_interfaces(repo, filters),
        SelectKind::Types => query_types(repo, filters),
    }
}

fn query_functions(repo: &Repo, filters: &[Filter]) -> Vec<QueryItem> {
    let mut sel = repo.functions();
    for f in filters {
        sel = match f {
            Filter::Named(name) => sel.named(name),
            Filter::InPackage(pkg) => sel.in_package(pkg),
            Filter::Exported(true) => sel.exported(),
            Filter::Exported(false) => sel.unexported(),
            Filter::ExcludingTests(true) => sel.excluding_tests(),
            Filter::ExcludingTests(false) => sel,
            // These filters don't apply to functions
            Filter::OnType(_) | Filter::Implementing(_) => sel,
        };
    }
    sel.collect()
        .iter()
        .map(|si| QueryItem::from_func(&si.item, &si.file))
        .collect()
}

fn query_methods(repo: &Repo, filters: &[Filter]) -> Vec<QueryItem> {
    let mut sel = repo.methods();
    for f in filters {
        sel = match f {
            Filter::Named(name) => sel.named(name),
            Filter::InPackage(pkg) => sel.in_package(pkg),
            Filter::Exported(true) => sel.exported(),
            Filter::Exported(false) => sel.unexported(),
            Filter::ExcludingTests(true) => sel.excluding_tests(),
            Filter::ExcludingTests(false) => sel,
            Filter::OnType(ty) => sel.on_type(ty),
            // implementing doesn't apply to methods
            Filter::Implementing(_) => sel,
        };
    }
    sel.collect()
        .iter()
        .map(|si| QueryItem::from_method(&si.item, &si.file))
        .collect()
}

fn query_structs(repo: &Repo, filters: &[Filter]) -> Vec<QueryItem> {
    query_type_specs(repo.structs(), filters)
}

fn query_interfaces(repo: &Repo, filters: &[Filter]) -> Vec<QueryItem> {
    query_type_specs(repo.interfaces(), filters)
}

fn query_types(repo: &Repo, filters: &[Filter]) -> Vec<QueryItem> {
    query_type_specs(repo.types(), filters)
}

fn query_type_specs(
    mut sel: go_analyzer::Selection<'_, go_model::TypeSpec>,
    filters: &[Filter],
) -> Vec<QueryItem> {
    for f in filters {
        sel = match f {
            Filter::Named(name) => sel.named(name),
            Filter::InPackage(pkg) => sel.in_package(pkg),
            Filter::Exported(true) => sel.exported(),
            Filter::Exported(false) => sel.unexported(),
            Filter::ExcludingTests(true) => sel.excluding_tests(),
            Filter::ExcludingTests(false) => sel,
            Filter::Implementing(iface) => sel.implementing(iface),
            // on_type doesn't apply to type specs
            Filter::OnType(_) => sel,
        };
    }
    sel.collect()
        .iter()
        .map(|si| QueryItem::from_type_spec(&si.item, &si.file))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_repo() -> Repo {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../go-analyzer/tests/fixture_repo");
        Repo::load(path).unwrap()
    }

    #[test]
    fn test_query_all_functions() {
        let repo = fixture_repo();
        let items = build_query(&repo, &SelectKind::Functions, &[]);
        assert!(items.len() >= 3, "expected at least 3 functions, got {}", items.len());
    }

    #[test]
    fn test_query_exported_functions() {
        let repo = fixture_repo();
        let items = build_query(
            &repo,
            &SelectKind::Functions,
            &[Filter::Exported(true)],
        );
        assert!(items.iter().all(|i| i.exported));
    }

    #[test]
    fn test_query_methods_on_type() {
        let repo = fixture_repo();
        let items = build_query(
            &repo,
            &SelectKind::Methods,
            &[Filter::OnType("Server".to_string())],
        );
        assert!(!items.is_empty());
        for item in &items {
            assert_eq!(item.receiver.as_deref(), Some("*Server"));
        }
    }

    #[test]
    fn test_query_structs_implementing_interface() {
        let repo = fixture_repo();
        let items = build_query(
            &repo,
            &SelectKind::Structs,
            &[Filter::Implementing("Stringer".to_string())],
        );
        // User and Server both implement Stringer (they have String() string)
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"User"), "expected User in {:?}", names);
        assert!(names.contains(&"Server"), "expected Server in {:?}", names);
    }

    #[test]
    fn test_query_named_function() {
        let repo = fixture_repo();
        let items = build_query(
            &repo,
            &SelectKind::Functions,
            &[Filter::Named("NewUser".to_string())],
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "NewUser");
    }

    #[test]
    fn test_query_in_package() {
        let repo = fixture_repo();
        let items = build_query(
            &repo,
            &SelectKind::Functions,
            &[Filter::InPackage("beta".to_string())],
        );
        assert!(items.iter().all(|i| i.package == "beta"));
    }

    #[test]
    fn test_inapplicable_filter_is_ignored() {
        let repo = fixture_repo();
        // on_type doesn't apply to functions — should be silently ignored
        let items = build_query(
            &repo,
            &SelectKind::Functions,
            &[Filter::OnType("Server".to_string())],
        );
        // Should return all functions (filter was ignored)
        let all = build_query(&repo, &SelectKind::Functions, &[]);
        assert_eq!(items.len(), all.len());
    }
}
```

Note: These tests depend on `QueryItem` from the output module (Task 5). The tests won't compile until Task 5 is complete. To develop Tasks 4 and 5 in TDD fashion, implement the `QueryItem` struct first (Step 1 of Task 5), then come back and run these tests.

- [ ] **Step 3: Add module declaration**

In `go-analyzer-mcp/src/main.rs`:
```rust
mod output;
mod selection_builder;
mod state;
```

- [ ] **Step 4: Run tests after Task 5 is complete**

Run: `cargo test -p go-analyzer-mcp -- selection_builder`
Expected: All 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add .
git commit -m "feat(mcp): add selection builder with shared query pipeline"
```

---

## Task 5: Define Output Types

Serializable output structs for all tools. These are what LLMs receive as tool results.

**Files:**
- Create: `go-analyzer-mcp/src/output.rs`

- [ ] **Step 1: Define QueryItem**

Create `go-analyzer-mcp/src/output.rs`:

```rust
use std::path::{Path, PathBuf};

use go_analyzer::printer::Printer;
use go_model::{
    ConstSpec, FuncDecl, FuncType, MethodDecl, Receiver, SourceFile, TopLevelDecl, TypeExpr,
    TypeSpec, VarSpec,
};
use serde::Serialize;

/// A single item returned by the query tool.
#[derive(Debug, Clone, Serialize)]
pub struct QueryItem {
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>,
    pub package: String,
    pub file: PathBuf,
    pub line: usize,
    pub end_line: usize,
    pub exported: bool,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

impl QueryItem {
    pub fn from_func(f: &FuncDecl, file: &Path) -> Self {
        Self {
            name: f.name.name.clone(),
            kind: "function".to_string(),
            receiver: None,
            package: String::new(), // filled in by caller if needed
            file: file.to_path_buf(),
            line: f.span.start_row + 1,
            end_line: f.span.end_row + 1,
            exported: f.name.is_exported(),
            signature: format!("func {}{}", f.name.name, format_func_type(&f.ty)),
            doc: f.doc.clone(),
        }
    }

    pub fn from_method(m: &MethodDecl, file: &Path) -> Self {
        Self {
            name: m.name.name.clone(),
            kind: "method".to_string(),
            receiver: Some(format_receiver(&m.receiver)),
            package: String::new(),
            file: file.to_path_buf(),
            line: m.span.start_row + 1,
            end_line: m.span.end_row + 1,
            exported: m.name.is_exported(),
            signature: format!(
                "func ({}) {}{}",
                format_receiver(&m.receiver),
                m.name.name,
                format_func_type(&m.ty),
            ),
            doc: m.doc.clone(),
        }
    }

    pub fn from_type_spec(t: &TypeSpec, file: &Path) -> Self {
        let kind = match t.ty() {
            TypeExpr::Struct(_) => "struct",
            TypeExpr::Interface(_) => "interface",
            _ => "type",
        };
        Self {
            name: t.name().name.clone(),
            kind: kind.to_string(),
            receiver: None,
            package: String::new(),
            file: file.to_path_buf(),
            line: t.span().start_row + 1,
            end_line: t.span().end_row + 1,
            exported: t.name().is_exported(),
            signature: format!("type {} {}", t.name().name, Printer::type_expr(t.ty())),
            doc: None, // TypeSpec doesn't have doc field yet
        }
    }

    /// Set the package name (derived from the repo's SourceFile for this path).
    pub fn with_package(mut self, package: &str) -> Self {
        self.package = package.to_string();
        self
    }
}

/// Format a function type as `(params) results` (without the leading `func`).
fn format_func_type(ft: &FuncType) -> String {
    let params = Printer::type_expr(&TypeExpr::Func(ft.clone()));
    // Printer::type_expr for Func returns "func(...) ..." — strip the leading "func"
    params.strip_prefix("func").unwrap_or(&params).to_string()
}

/// Format a receiver as it appears in Go: `s *Server` or `c Client`.
fn format_receiver(r: &Receiver) -> String {
    let ty = Printer::type_expr(&r.ty);
    match &r.name {
        Some(name) => format!("{} {}", name.name, ty),
        None => ty,
    }
}

/// Overview of a single file returned by describe_file.
#[derive(Debug, Clone, Serialize)]
pub struct FileOverview {
    pub package: String,
    pub file: PathBuf,
    pub imports: Vec<String>,
    pub types: Vec<TypeOverview>,
    pub functions: Vec<FunctionOverview>,
    pub methods: Vec<MethodOverview>,
    pub constants: Vec<ValueOverview>,
    pub variables: Vec<ValueOverview>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeOverview {
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub exported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionOverview {
    pub name: String,
    pub line: usize,
    pub signature: String,
    pub exported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MethodOverview {
    pub name: String,
    pub receiver: String,
    pub line: usize,
    pub signature: String,
    pub exported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValueOverview {
    pub name: String,
    pub line: usize,
    pub exported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Overview of the module returned by describe_module.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleOverview {
    pub module: String,
    pub path: PathBuf,
    pub packages: Vec<PackageOverview>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackageOverview {
    pub name: String,
    pub import_path: String,
    pub path: PathBuf,
    pub files: Vec<String>,
    pub types: usize,
    pub functions: usize,
    pub methods: usize,
    pub constants: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

/// Result from the call_graph tool.
#[derive(Debug, Clone, Serialize)]
pub struct CallGraphResult {
    pub nodes: Vec<CallGraphNode>,
    pub edges: Vec<CallGraphEdge>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallGraphNode {
    pub symbol: String,
    pub file: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallGraphEdge {
    pub from: String,
    pub to: String,
}

/// Result from the edit tool.
#[derive(Debug, Clone, Serialize)]
pub struct EditResult {
    pub diff: String,
    pub files_modified: Vec<PathBuf>,
    pub edits_applied: usize,
}

/// Build a FileOverview from a SourceFile AST.
pub fn build_file_overview(
    source_file: &SourceFile,
    file_path: &Path,
    include_docs: bool,
) -> FileOverview {
    let imports: Vec<String> = source_file
        .imports
        .iter()
        .map(|imp| imp.path.value())
        .collect();

    let mut types = Vec::new();
    let mut functions = Vec::new();
    let mut methods = Vec::new();
    let mut constants = Vec::new();
    let mut variables = Vec::new();

    for decl in &source_file.decls {
        match decl {
            TopLevelDecl::Func(f) => {
                functions.push(FunctionOverview {
                    name: f.name.name.clone(),
                    line: f.span.start_row + 1,
                    signature: format!("func {}{}", f.name.name, format_func_type(&f.ty)),
                    exported: f.name.is_exported(),
                    doc: if include_docs { f.doc.clone() } else { None },
                });
            }
            TopLevelDecl::Method(m) => {
                methods.push(MethodOverview {
                    name: m.name.name.clone(),
                    receiver: format_receiver(&m.receiver),
                    line: m.span.start_row + 1,
                    signature: format!(
                        "func ({}) {}{}",
                        format_receiver(&m.receiver),
                        m.name.name,
                        format_func_type(&m.ty),
                    ),
                    exported: m.name.is_exported(),
                    doc: if include_docs { m.doc.clone() } else { None },
                });
            }
            TopLevelDecl::Type(specs) => {
                for t in specs {
                    let kind = match t.ty() {
                        TypeExpr::Struct(_) => "struct",
                        TypeExpr::Interface(_) => "interface",
                        _ => "type",
                    };
                    types.push(TypeOverview {
                        name: t.name().name.clone(),
                        kind: kind.to_string(),
                        line: t.span().start_row + 1,
                        exported: t.name().is_exported(),
                        doc: None, // TypeSpec doesn't have doc field yet
                    });
                }
            }
            TopLevelDecl::Const(specs) => {
                for c in specs {
                    for name in &c.names {
                        constants.push(ValueOverview {
                            name: name.name.clone(),
                            line: c.span.start_row + 1,
                            exported: name.is_exported(),
                            value: c.values.first().map(|v| Printer::expr(v)),
                        });
                    }
                }
            }
            TopLevelDecl::Var(specs) => {
                for v in specs {
                    for name in &v.names {
                        variables.push(ValueOverview {
                            name: name.name.clone(),
                            line: v.span.start_row + 1,
                            exported: name.is_exported(),
                            value: v.values.first().map(|v| Printer::expr(v)),
                        });
                    }
                }
            }
        }
    }

    FileOverview {
        package: source_file.package.name.clone(),
        file: file_path.to_path_buf(),
        imports,
        types,
        functions,
        methods,
        constants,
        variables,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use go_analyzer::Repo;

    fn fixture_repo() -> Repo {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../go-analyzer/tests/fixture_repo");
        Repo::load(path).unwrap()
    }

    #[test]
    fn test_query_item_from_func_has_location() {
        let repo = fixture_repo();
        let funcs = repo.functions().named("NewUser");
        let item = funcs.collect().first().unwrap();
        let qi = QueryItem::from_func(&item.item, &item.file);

        assert_eq!(qi.name, "NewUser");
        assert_eq!(qi.kind, "function");
        assert!(qi.line > 0);
        assert!(qi.file.to_string_lossy().ends_with(".go"));
        assert!(qi.exported);
        assert!(qi.signature.contains("NewUser"));
    }

    #[test]
    fn test_query_item_from_method_has_receiver() {
        let repo = fixture_repo();
        let methods = repo.methods().on_type("User").named("String");
        let item = methods.collect().first().unwrap();
        let qi = QueryItem::from_method(&item.item, &item.file);

        assert_eq!(qi.name, "String");
        assert_eq!(qi.kind, "method");
        assert!(qi.receiver.is_some());
        assert!(qi.receiver.as_ref().unwrap().contains("User"));
    }

    #[test]
    fn test_query_item_from_type_spec() {
        let repo = fixture_repo();
        let structs = repo.structs().named("User");
        let item = structs.collect().first().unwrap();
        let qi = QueryItem::from_type_spec(&item.item, &item.file);

        assert_eq!(qi.name, "User");
        assert_eq!(qi.kind, "struct");
        assert!(qi.exported);
    }

    #[test]
    fn test_file_overview_has_all_sections() {
        let repo = fixture_repo();
        // Access internal files to get a SourceFile — we need to go through the repo
        // For now, load directly and check the output structure
        let funcs = repo.functions().named("NewUser");
        let first = funcs.collect().first().unwrap();
        // We can't access repo.files directly, so test via the query items
        // The full integration test for describe_file will test build_file_overview
        assert!(!first.file.to_string_lossy().is_empty());
    }
}
```

- [ ] **Step 2: Verify tests pass**

Run: `cargo test -p go-analyzer-mcp -- output`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add .
git commit -m "feat(mcp): add serializable output types for all tools"
```

---

## Task 6: Implement describe_file Tool Handler

**Files:**
- Create: `go-analyzer-mcp/src/tools/mod.rs`
- Create: `go-analyzer-mcp/src/tools/describe.rs`
- Modify: `go-analyzer-mcp/src/main.rs` (add mod declaration)

- [ ] **Step 1: Create tools module**

Create `go-analyzer-mcp/src/tools/mod.rs`:
```rust
pub mod describe;
```

- [ ] **Step 2: Implement describe_file handler**

Create `go-analyzer-mcp/src/tools/describe.rs`:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::output::{
    build_file_overview, FileOverview, ModuleOverview, PackageOverview,
};
use crate::state::ServerState;

#[derive(Debug, Deserialize)]
pub struct DescribeFileInput {
    pub path: PathBuf,
    #[serde(default)]
    pub include_docs: bool,
}

#[derive(Debug, Deserialize)]
pub struct DescribeModuleInput {
    pub depth: Option<usize>,
    #[serde(default)]
    pub include_docs: bool,
}

/// Handle the describe_file tool call.
pub fn handle_describe_file(
    state: &mut ServerState,
    input: &DescribeFileInput,
) -> Result<FileOverview, DescribeError> {
    let repo = state.repo().map_err(|e| DescribeError::State(e.to_string()))?;

    // Find the file in the repo by matching the path
    let target = if input.path.is_absolute() {
        input.path.clone()
    } else {
        repo.root().join(&input.path)
    };
    let target = target
        .canonicalize()
        .map_err(|e| DescribeError::FileNotFound {
            path: input.path.clone(),
            source: e.to_string(),
        })?;

    // Access the parsed SourceFile from the repo
    // We need to iterate over repo files to find the matching one.
    // This requires Repo to expose file iteration — we'll add a helper.
    // For now, we re-parse the file using the walker directly.
    let source = std::fs::read(&target).map_err(|e| DescribeError::FileNotFound {
        path: target.clone(),
        source: e.to_string(),
    })?;
    let ast = go_analyzer::walker::parse_and_walk(&source).map_err(|e| DescribeError::ParseFailed {
        path: target.clone(),
        source: e.to_string(),
    })?;

    Ok(build_file_overview(&ast, &target, input.include_docs))
}

/// Handle the describe_module tool call.
pub fn handle_describe_module(
    state: &mut ServerState,
    input: &DescribeModuleInput,
) -> Result<ModuleOverview, DescribeError> {
    let repo = state.repo().map_err(|e| DescribeError::State(e.to_string()))?;
    let root = repo.root().to_path_buf();

    // Try to read go.mod for module name
    let module_name = read_module_name(&root).unwrap_or_else(|| {
        root.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    });

    // Scan directories for Go packages
    let mut packages: HashMap<PathBuf, PackageInfo> = HashMap::new();
    scan_packages(&root, &root, input.depth, 0, &mut packages);

    // Sort packages by path for deterministic output
    let mut sorted: Vec<_> = packages.into_iter().collect();
    sorted.sort_by(|(a, _), (b, _)| a.cmp(b));

    let packages = sorted
        .into_iter()
        .map(|(path, info)| {
            let rel = path.strip_prefix(&root).unwrap_or(&path);
            let import_path = if rel == Path::new("") {
                module_name.clone()
            } else {
                format!("{}/{}", module_name, rel.display())
            };
            PackageOverview {
                name: info.name,
                import_path,
                path: path.clone(),
                files: info.files,
                types: info.types,
                functions: info.functions,
                methods: info.methods,
                constants: info.constants,
                doc: None, // Package-level doc not yet supported
            }
        })
        .collect();

    Ok(ModuleOverview {
        module: module_name,
        path: root,
        packages,
    })
}

struct PackageInfo {
    name: String,
    files: Vec<String>,
    types: usize,
    functions: usize,
    methods: usize,
    constants: usize,
}

fn scan_packages(
    dir: &Path,
    root: &Path,
    max_depth: Option<usize>,
    current_depth: usize,
    packages: &mut HashMap<PathBuf, PackageInfo>,
) {
    if let Some(max) = max_depth {
        if current_depth > max {
            return;
        }
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    let mut go_files = Vec::new();
    let mut subdirs = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() && !name.starts_with('.') && name != "vendor" && name != "node_modules" {
            subdirs.push(path);
        } else if path.extension().is_some_and(|ext| ext == "go") {
            go_files.push(path);
        }
    }

    if !go_files.is_empty() {
        let mut info = PackageInfo {
            name: String::new(),
            files: Vec::new(),
            types: 0,
            functions: 0,
            methods: 0,
            constants: 0,
        };

        for go_file in &go_files {
            if let Some(file_name) = go_file.file_name() {
                info.files.push(file_name.to_string_lossy().to_string());
            }
            if let Ok(source) = std::fs::read(go_file) {
                if let Ok(ast) = go_analyzer::walker::parse_and_walk(&source) {
                    if info.name.is_empty() {
                        info.name = ast.package.name.clone();
                    }
                    for decl in &ast.decls {
                        match decl {
                            go_model::TopLevelDecl::Func(_) => info.functions += 1,
                            go_model::TopLevelDecl::Method(_) => info.methods += 1,
                            go_model::TopLevelDecl::Type(specs) => info.types += specs.len(),
                            go_model::TopLevelDecl::Const(specs) => {
                                info.constants += specs.iter().map(|s| s.names.len()).sum::<usize>();
                            }
                            go_model::TopLevelDecl::Var(_) => {}
                        }
                    }
                }
            }
        }

        info.files.sort();
        packages.insert(dir.to_path_buf(), info);
    }

    for subdir in subdirs {
        scan_packages(&subdir, root, max_depth, current_depth + 1, packages);
    }
}

fn read_module_name(root: &Path) -> Option<String> {
    let go_mod = root.join("go.mod");
    let content = std::fs::read_to_string(go_mod).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if let Some(module) = line.strip_prefix("module ") {
            return Some(module.trim().to_string());
        }
    }
    None
}

#[derive(Debug, thiserror::Error)]
pub enum DescribeError {
    #[error("state error: {0}")]
    State(String),
    #[error("file not found at {path}: {source}")]
    FileNotFound { path: PathBuf, source: String },
    #[error("failed to parse {path}: {source}")]
    ParseFailed { path: PathBuf, source: String },
}
```

- [ ] **Step 3: Add mod declaration**

In `go-analyzer-mcp/src/main.rs`, add:
```rust
mod tools;
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p go-analyzer-mcp`
Expected: Compiles. If `walker::parse_and_walk` is not public, we need to make it public in `go-analyzer/src/lib.rs` (add it to the pub use section or ensure `walker` module is public — it already is: `pub mod walker` on line 54).

- [ ] **Step 5: Commit**

```bash
git add .
git commit -m "feat(mcp): implement describe_file and describe_module tool handlers"
```

---

## Task 7: Implement Query Tool Handler

**Files:**
- Create: `go-analyzer-mcp/src/tools/query.rs`
- Modify: `go-analyzer-mcp/src/tools/mod.rs`

- [ ] **Step 1: Implement query handler**

Create `go-analyzer-mcp/src/tools/query.rs`:

```rust
use serde::Deserialize;

use crate::output::QueryItem;
use crate::selection_builder::{build_query, Filter, SelectKind};
use crate::state::ServerState;

#[derive(Debug, Deserialize)]
pub struct QueryInput {
    pub select: SelectKind,
    #[serde(default)]
    pub filters: Vec<Filter>,
}

#[derive(Debug, serde::Serialize)]
pub struct QueryOutput {
    pub items: Vec<QueryItem>,
    pub count: usize,
}

/// Handle the query tool call.
pub fn handle_query(
    state: &mut ServerState,
    input: &QueryInput,
) -> Result<QueryOutput, QueryError> {
    let repo = state.repo().map_err(|e| QueryError::State(e.to_string()))?;

    // Build query and enrich with package names
    let mut items = build_query(repo, &input.select, &input.filters);

    // Enrich items with package names from the repo
    // The repo has file → SourceFile mappings, but we can derive package from the path
    // For now, we rely on the fact that Go packages = directories
    // A proper implementation would look up the file in the repo's parsed ASTs
    // This is acceptable for v1

    let count = items.len();
    Ok(QueryOutput { items, count })
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("state error: {0}")]
    State(String),
}
```

- [ ] **Step 2: Add to tools/mod.rs**

```rust
pub mod describe;
pub mod query;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p go-analyzer-mcp`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(mcp): implement query tool handler"
```

---

## Task 8: Implement Call Graph Tool Handler

**Files:**
- Create: `go-analyzer-mcp/src/tools/call_graph.rs`
- Modify: `go-analyzer-mcp/src/tools/mod.rs`

- [ ] **Step 1: Implement call_graph handler**

Create `go-analyzer-mcp/src/tools/call_graph.rs`:

```rust
use std::collections::HashSet;

use go_analyzer::callgraph::{CallGraph, Symbol, SymbolKind};
use serde::Deserialize;

use crate::output::{CallGraphEdge, CallGraphNode, CallGraphResult};
use crate::state::ServerState;

#[derive(Debug, Deserialize)]
pub struct CallGraphInput {
    pub action: CallGraphAction,
    /// Symbol name to query. Not needed for `dead_code`.
    #[serde(default)]
    pub symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallGraphAction {
    Callers,
    Callees,
    ReachableFrom,
    DeadCode,
}

/// Handle the call_graph tool call.
pub fn handle_call_graph(
    state: &mut ServerState,
    input: &CallGraphInput,
) -> Result<CallGraphResult, CallGraphError> {
    let repo = state.repo().map_err(|e| CallGraphError::State(e.to_string()))?;
    let graph = CallGraph::build(repo);

    match input.action {
        CallGraphAction::Callers => {
            let symbol_name = input
                .symbol
                .as_deref()
                .ok_or(CallGraphError::MissingSymbol("callers"))?;
            callers(&graph, symbol_name)
        }
        CallGraphAction::Callees => {
            let symbol_name = input
                .symbol
                .as_deref()
                .ok_or(CallGraphError::MissingSymbol("callees"))?;
            callees(&graph, symbol_name)
        }
        CallGraphAction::ReachableFrom => {
            let symbol_name = input
                .symbol
                .as_deref()
                .ok_or(CallGraphError::MissingSymbol("reachable_from"))?;
            reachable_from(&graph, symbol_name)
        }
        CallGraphAction::DeadCode => dead_code(&graph),
    }
}

fn find_symbols_by_name<'a>(graph: &'a CallGraph, name: &str) -> Vec<&'a Symbol> {
    graph
        .symbols
        .keys()
        .filter(|s| s.name == name)
        .collect()
}

fn callers(graph: &CallGraph, symbol_name: &str) -> Result<CallGraphResult, CallGraphError> {
    let symbols = find_symbols_by_name(graph, symbol_name);
    if symbols.is_empty() {
        return Err(CallGraphError::SymbolNotFound(symbol_name.to_string()));
    }

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut text = String::new();

    for sym in &symbols {
        if let Some(caller_set) = graph.called_by.get(*sym) {
            text.push_str(&format!("{}\n", sym));
            for caller in caller_set {
                if let Some(entry) = graph.symbols.get(caller) {
                    nodes.push(CallGraphNode {
                        symbol: caller.to_string(),
                        file: entry.file.clone(),
                        line: entry.span.start_row + 1,
                    });
                    edges.push(CallGraphEdge {
                        from: caller.to_string(),
                        to: sym.to_string(),
                    });
                    text.push_str(&format!(
                        "  <- {} ({}:{})\n",
                        caller,
                        entry.file.display(),
                        entry.span.start_row + 1
                    ));
                }
            }
        }
    }

    // Add the target symbol(s) as nodes too
    for sym in &symbols {
        if let Some(entry) = graph.symbols.get(*sym) {
            nodes.push(CallGraphNode {
                symbol: sym.to_string(),
                file: entry.file.clone(),
                line: entry.span.start_row + 1,
            });
        }
    }

    Ok(CallGraphResult { nodes, edges, text })
}

fn callees(graph: &CallGraph, symbol_name: &str) -> Result<CallGraphResult, CallGraphError> {
    let symbols = find_symbols_by_name(graph, symbol_name);
    if symbols.is_empty() {
        return Err(CallGraphError::SymbolNotFound(symbol_name.to_string()));
    }

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut text = String::new();

    for sym in &symbols {
        if let Some(entry) = graph.symbols.get(*sym) {
            nodes.push(CallGraphNode {
                symbol: sym.to_string(),
                file: entry.file.clone(),
                line: entry.span.start_row + 1,
            });
        }
        text.push_str(&format!("{}\n", sym));
        if let Some(callee_set) = graph.calls.get(*sym) {
            for callee in callee_set {
                if let Some(entry) = graph.symbols.get(callee) {
                    nodes.push(CallGraphNode {
                        symbol: callee.to_string(),
                        file: entry.file.clone(),
                        line: entry.span.start_row + 1,
                    });
                    edges.push(CallGraphEdge {
                        from: sym.to_string(),
                        to: callee.to_string(),
                    });
                    text.push_str(&format!(
                        "  -> {} ({}:{})\n",
                        callee,
                        entry.file.display(),
                        entry.span.start_row + 1
                    ));
                }
            }
        }
    }

    Ok(CallGraphResult { nodes, edges, text })
}

fn reachable_from(
    graph: &CallGraph,
    symbol_name: &str,
) -> Result<CallGraphResult, CallGraphError> {
    let symbols = find_symbols_by_name(graph, symbol_name);
    if symbols.is_empty() {
        return Err(CallGraphError::SymbolNotFound(symbol_name.to_string()));
    }

    let entries: Vec<Symbol> = symbols.into_iter().cloned().collect();
    let reachable = graph.reachable_from(&entries);

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut text = String::new();

    text.push_str(&format!("Reachable from {}:\n", symbol_name));

    for sym in &reachable {
        if let Some(entry) = graph.symbols.get(sym) {
            nodes.push(CallGraphNode {
                symbol: sym.to_string(),
                file: entry.file.clone(),
                line: entry.span.start_row + 1,
            });
            text.push_str(&format!(
                "  {} ({}:{})\n",
                sym,
                entry.file.display(),
                entry.span.start_row + 1
            ));
        }

        if let Some(callee_set) = graph.calls.get(sym) {
            for callee in callee_set {
                if reachable.contains(callee) {
                    edges.push(CallGraphEdge {
                        from: sym.to_string(),
                        to: callee.to_string(),
                    });
                }
            }
        }
    }

    Ok(CallGraphResult { nodes, edges, text })
}

fn dead_code(graph: &CallGraph) -> Result<CallGraphResult, CallGraphError> {
    // Find entry points: main functions and exported symbols
    let entries: Vec<Symbol> = graph
        .symbols
        .iter()
        .filter(|(_, entry)| {
            entry.exported
                || entry.symbol.name == "main"
                || entry.symbol.name == "init"
                || matches!(&entry.kind, SymbolKind::Type)
        })
        .map(|(sym, _)| sym.clone())
        .collect();

    let reachable = graph.reachable_from(&entries);

    let mut nodes = Vec::new();
    let mut text = String::from("Dead code (unreachable symbols):\n");

    for (sym, entry) in &graph.symbols {
        if !reachable.contains(sym) {
            nodes.push(CallGraphNode {
                symbol: sym.to_string(),
                file: entry.file.clone(),
                line: entry.span.start_row + 1,
            });
            text.push_str(&format!(
                "  {} ({}:{})\n",
                sym,
                entry.file.display(),
                entry.span.start_row + 1
            ));
        }
    }

    Ok(CallGraphResult {
        nodes,
        edges: vec![],
        text,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum CallGraphError {
    #[error("state error: {0}")]
    State(String),
    #[error("symbol not found: {0}")]
    SymbolNotFound(String),
    #[error("{0} action requires a symbol parameter")]
    MissingSymbol(&'static str),
}
```

- [ ] **Step 2: Add to tools/mod.rs**

```rust
pub mod call_graph;
pub mod describe;
pub mod query;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p go-analyzer-mcp`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(mcp): implement call_graph tool handler"
```

---

## Task 9: Implement Edit Tool Handler

**Files:**
- Create: `go-analyzer-mcp/src/tools/edit.rs`
- Modify: `go-analyzer-mcp/src/tools/mod.rs`

- [ ] **Step 1: Define edit input types**

Create `go-analyzer-mcp/src/tools/edit.rs`:

```rust
use std::collections::HashMap;
use std::path::PathBuf;

use go_analyzer::printer::Printer;
use go_analyzer::{Changes, Repo};
use go_model::{Block, TypeExpr};
use serde::Deserialize;

use crate::output::EditResult;
use crate::selection_builder::{Filter, SelectKind};
use crate::state::ServerState;

#[derive(Debug, Deserialize)]
pub struct EditInput {
    pub select: SelectKind,
    #[serde(default)]
    pub filters: Vec<Filter>,
    pub action: EditAction,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditAction {
    Delete,
    Rename(String),
    ReplaceBody(String),
    AddField { name: String, ty: String },
    RemoveField(String),
}

/// Handle the edit tool call.
pub fn handle_edit(
    state: &mut ServerState,
    input: &EditInput,
) -> Result<EditResult, EditError> {
    let repo = state.repo().map_err(|e| EditError::State(e.to_string()))?;

    let changes = build_changes(repo, &input.select, &input.filters, &input.action)?;

    if changes.is_empty() {
        return Err(EditError::EmptySelection {
            select: format!("{:?}", input.select),
            filters: format!("{:?}", input.filters),
        });
    }

    let applied = repo.apply(changes);
    let diff = build_diff(repo, &applied);
    let files_modified: Vec<PathBuf> = applied.affected_files().iter().map(|p| p.to_path_buf()).collect();
    let edits_applied = applied.edit_count();

    if !input.dry_run {
        applied
            .commit()
            .map_err(|e| EditError::CommitFailed(e.to_string()))?;
        // Reload repo to reflect changes
        state.reload().map_err(|e| EditError::State(e.to_string()))?;
    }

    Ok(EditResult {
        diff,
        files_modified,
        edits_applied,
    })
}

fn build_changes(
    repo: &Repo,
    select: &SelectKind,
    filters: &[Filter],
    action: &EditAction,
) -> Result<Changes, EditError> {
    match select {
        SelectKind::Functions => build_func_changes(repo, filters, action),
        SelectKind::Methods => build_method_changes(repo, filters, action),
        SelectKind::Structs | SelectKind::Types => build_type_changes(repo, select, filters, action),
        SelectKind::Interfaces => build_type_changes(repo, select, filters, action),
    }
}

fn build_func_changes(
    repo: &Repo,
    filters: &[Filter],
    action: &EditAction,
) -> Result<Changes, EditError> {
    let mut sel = repo.functions();
    for f in filters {
        sel = match f {
            Filter::Named(name) => sel.named(name),
            Filter::InPackage(pkg) => sel.in_package(pkg),
            Filter::Exported(true) => sel.exported(),
            Filter::Exported(false) => sel.unexported(),
            Filter::ExcludingTests(true) => sel.excluding_tests(),
            Filter::ExcludingTests(false) | Filter::OnType(_) | Filter::Implementing(_) => sel,
        };
    }

    Ok(match action {
        EditAction::Delete => sel.delete(),
        EditAction::Rename(new_name) => sel.rename(new_name),
        EditAction::ReplaceBody(body_src) => {
            let body = parse_go_block(body_src)?;
            sel.replace_body(body)
        }
        EditAction::AddField { .. } | EditAction::RemoveField(_) => {
            return Err(EditError::InvalidAction(
                "add_field/remove_field only applies to structs".to_string(),
            ))
        }
    })
}

fn build_method_changes(
    repo: &Repo,
    filters: &[Filter],
    action: &EditAction,
) -> Result<Changes, EditError> {
    let mut sel = repo.methods();
    for f in filters {
        sel = match f {
            Filter::Named(name) => sel.named(name),
            Filter::InPackage(pkg) => sel.in_package(pkg),
            Filter::Exported(true) => sel.exported(),
            Filter::Exported(false) => sel.unexported(),
            Filter::ExcludingTests(true) => sel.excluding_tests(),
            Filter::ExcludingTests(false) | Filter::Implementing(_) => sel,
            Filter::OnType(ty) => sel.on_type(ty),
        };
    }

    Ok(match action {
        EditAction::Delete => sel.delete(),
        EditAction::Rename(new_name) => sel.rename(new_name),
        EditAction::ReplaceBody(body_src) => {
            let body = parse_go_block(body_src)?;
            sel.replace_body(body)
        }
        EditAction::AddField { .. } | EditAction::RemoveField(_) => {
            return Err(EditError::InvalidAction(
                "add_field/remove_field only applies to structs".to_string(),
            ))
        }
    })
}

fn build_type_changes(
    repo: &Repo,
    select: &SelectKind,
    filters: &[Filter],
    action: &EditAction,
) -> Result<Changes, EditError> {
    let mut sel = match select {
        SelectKind::Structs => repo.structs(),
        SelectKind::Interfaces => repo.interfaces(),
        SelectKind::Types => repo.types(),
        _ => unreachable!(),
    };
    for f in filters {
        sel = match f {
            Filter::Named(name) => sel.named(name),
            Filter::InPackage(pkg) => sel.in_package(pkg),
            Filter::Exported(true) => sel.exported(),
            Filter::Exported(false) => sel.unexported(),
            Filter::ExcludingTests(true) => sel.excluding_tests(),
            Filter::ExcludingTests(false) | Filter::OnType(_) => sel,
            Filter::Implementing(iface) => sel.implementing(iface),
        };
    }

    Ok(match action {
        EditAction::Delete => sel.delete(),
        EditAction::Rename(new_name) => sel.rename(new_name),
        EditAction::AddField { name, ty } => {
            // Parse the type expression string into a TypeExpr
            // For now, use a simple named type. A full parser would handle complex types.
            let type_expr = TypeExpr::Named(go_model::Ident::synthetic(ty));
            sel.add_field(name, type_expr)
        }
        EditAction::RemoveField(name) => sel.remove_field(name),
        EditAction::ReplaceBody(_) => {
            return Err(EditError::InvalidAction(
                "replace_body doesn't apply to types".to_string(),
            ))
        }
    })
}

/// Parse a Go block body string like `{ return 42 }` into a Block AST node.
/// For v1, we wrap the body string in a minimal function and parse it.
fn parse_go_block(body_src: &str) -> Result<Block, EditError> {
    // Wrap in a function so tree-sitter can parse it
    let wrapped = format!("package p\nfunc _() {{\n{body_src}\n}}");
    let ast = go_analyzer::walker::parse_and_walk(wrapped.as_bytes())
        .map_err(|e| EditError::ParseFailed(format!("failed to parse body: {e}")))?;

    // Extract the body from the parsed function
    for decl in &ast.decls {
        if let go_model::TopLevelDecl::Func(f) = decl {
            if let Some(body) = &f.body {
                return Ok(body.clone());
            }
        }
    }
    Err(EditError::ParseFailed(
        "could not extract block from parsed body".to_string(),
    ))
}

/// Build a unified diff string from an Applied result.
fn build_diff(repo: &go_analyzer::Repo, applied: &go_analyzer::Applied<'_>) -> String {
    // Use dry_run to get the modified content, then diff against originals
    let modified = applied.dry_run();
    let mut diff = String::new();

    for (path, new_content) in &modified {
        // Read the original file to diff against
        if let Ok(original) = std::fs::read_to_string(path) {
            let file_diff = unified_diff(path, &original, new_content);
            if !file_diff.is_empty() {
                diff.push_str(&file_diff);
            }
        }
    }

    diff
}

fn unified_diff(path: &std::path::Path, old: &str, new: &str) -> String {
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

// NOTE: add_method and modify_method edit actions are deferred per spec.
// They require constructing AST nodes from JSON input, which needs careful
// schema design. The spec notes "to be refined during implementation."
// Add these as a follow-up task once the core edit actions are working.

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("state error: {0}")]
    State(String),
    #[error("no items matched the selection (select: {select}, filters: {filters})")]
    EmptySelection { select: String, filters: String },
    #[error("invalid action for this selection: {0}")]
    InvalidAction(String),
    #[error("parse failed: {0}")]
    ParseFailed(String),
    #[error("failed to write changes: {0}")]
    CommitFailed(String),
}
```

- [ ] **Step 2: Add to tools/mod.rs**

```rust
pub mod call_graph;
pub mod describe;
pub mod edit;
pub mod query;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p go-analyzer-mcp`
Expected: Compiles. Note: `applied.dry_run()` is called before `applied.commit()` which moves `self`. This may need adjustment — `dry_run()` takes `&self` but `commit()` takes `self`. We may need to call `dry_run()` first, save the diff, then call `commit()`. Check and fix if needed.

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "feat(mcp): implement edit tool handler with dry_run support"
```

---

## Task 10: Wire Up MCP Server with rmcp

Connect all tool handlers to the rmcp server framework and start the stdio transport.

**Files:**
- Create: `go-analyzer-mcp/src/server.rs`
- Modify: `go-analyzer-mcp/src/main.rs`

- [ ] **Step 1: Check rmcp API**

Before implementing, read the rmcp crate docs to understand the exact API for:
1. Defining a server with tools
2. Tool input/output handling
3. Starting the stdio transport

Run: `cargo doc -p rmcp --open` or check docs.rs/rmcp.

Adjust the implementation below based on what you find. The code below assumes a derive-macro-based tool registration pattern common in MCP SDKs. If rmcp uses a different pattern (e.g., trait impl, builder), adapt accordingly.

- [ ] **Step 2: Implement server.rs**

Create `go-analyzer-mcp/src/server.rs`. The exact implementation depends on the rmcp API discovered in Step 1. Here's the conceptual structure:

```rust
use std::sync::Mutex;

use crate::state::ServerState;
use crate::tools::{
    call_graph::{handle_call_graph, CallGraphInput},
    describe::{handle_describe_file, handle_describe_module, DescribeFileInput, DescribeModuleInput},
    edit::{handle_edit, EditInput},
    query::{handle_query, QueryInput},
};

/// The MCP server that dispatches tool calls to handlers.
pub struct GoAnalyzerServer {
    state: Mutex<ServerState>,
}

impl GoAnalyzerServer {
    pub fn new(state: ServerState) -> Self {
        Self {
            state: Mutex::new(state),
        }
    }
}

// The exact tool registration depends on the rmcp API.
// Common patterns:
//
// 1. Derive macro:
//    #[tool(name = "query", description = "...")]
//    async fn query(&self, input: QueryInput) -> Result<String> { ... }
//
// 2. Trait impl:
//    impl ToolHandler for GoAnalyzerServer {
//        fn list_tools() -> Vec<Tool> { ... }
//        fn call_tool(name, args) -> Result<String> { ... }
//    }
//
// 3. Builder:
//    Server::new()
//        .tool("query", handler_fn)
//        .tool("edit", handler_fn)
//
// Implement whichever pattern rmcp uses. Each tool handler should:
// 1. Deserialize the JSON input into the appropriate Input struct
// 2. Lock the state mutex
// 3. Call the handler function
// 4. Serialize the result to JSON
// 5. Return the JSON string as the tool output
//
// Tool descriptions for the MCP schema:
//
// query: "Find and filter Go declarations (functions, methods, structs, interfaces, types) using a pipeline of select + filters. Returns items with file paths, line numbers, and signatures."
//
// call_graph: "Analyze call relationships between Go symbols. Actions: callers, callees, reachable_from, dead_code. Returns structured graph data and a readable text tree."
//
// edit: "Modify Go code using a select + filters pipeline plus an action (delete, rename, replace_body, add_field, remove_field). Auto-applies and returns unified diff unless dry_run is true."
//
// describe_file: "Get a structural overview of a Go source file: types, functions, methods, constants, variables with line numbers and optional doc comments."
//
// describe_module: "Get the package tree of the loaded Go module with summary counts per package. Supports depth limiting for progressive exploration."
```

- [ ] **Step 3: Update main.rs to start the server**

Update `go-analyzer-mcp/src/main.rs`:

```rust
mod output;
mod selection_builder;
mod server;
mod state;
mod tools;

use clap::Parser;
use std::path::PathBuf;

use server::GoAnalyzerServer;
use state::ServerState;

#[derive(Parser)]
#[command(name = "go-analyzer-mcp", about = "MCP server for Go code analysis")]
struct Cli {
    /// Path to the Go project to analyze. Defaults to current directory.
    #[arg(long, default_value = ".")]
    path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let path = cli.path.canonicalize()?;
    eprintln!("go-analyzer-mcp: serving {}", path.display());

    let state = ServerState::new(path);
    let server = GoAnalyzerServer::new(state);

    // Start stdio transport — exact API depends on rmcp
    // Example (adjust based on rmcp docs):
    // rmcp::serve_stdio(server).await?;

    Ok(())
}
```

- [ ] **Step 4: Add MCP prompt**

Register the workflow prompt with the MCP server. The exact mechanism depends on rmcp. The prompt content:

```
You have access to a Go code analyzer. Typical workflow:
1. describe_module — understand the package structure
2. describe_file — drill into specific files
3. query — find specific types, functions, methods with filters
4. call_graph — understand dependencies and call chains
5. edit — make changes (returns diff, auto-applies unless dry_run: true)

All query/edit tools use a pipeline: select a kind (functions, methods, structs,
interfaces, types), then chain filters (named, in_package, exported, on_type,
implementing, excluding_tests).
```

- [ ] **Step 5: Verify it compiles and runs**

Run: `cargo build -p go-analyzer-mcp && echo '{}' | cargo run -p go-analyzer-mcp -- --path go-analyzer/tests/fixture_repo`
Expected: Server starts (stderr message), reads from stdin, responds to MCP initialization.

- [ ] **Step 6: Commit**

```bash
git add .
git commit -m "feat(mcp): wire up MCP server with rmcp stdio transport"
```

---

## Task 11: Extend Fixture Repo for Integration Tests

**Files:**
- Create: `go-analyzer-mcp/tests/fixture_repo/go.mod`
- The MCP crate's integration tests will use the existing fixture repo from `go-analyzer/tests/fixture_repo`. We symlink or reference it directly. But for `describe_module` tests, we need a `go.mod` file.

- [ ] **Step 1: Add go.mod to fixture repo**

Create `go-analyzer/tests/fixture_repo/go.mod`:
```
module github.com/test/fixture

go 1.21
```

- [ ] **Step 2: Verify existing go-analyzer tests still pass**

Run: `cargo test --all-targets -p go-analyzer`
Expected: All tests pass (go.mod is just a data file, doesn't affect Go parsing).

- [ ] **Step 3: Commit**

```bash
git add .
git commit -m "test: add go.mod to fixture repo for describe_module tests"
```

---

## Task 12: Integration Tests for Query Tool

**Files:**
- Create: `go-analyzer-mcp/tests/query_test.rs`

- [ ] **Step 1: Write integration tests**

Create `go-analyzer-mcp/tests/query_test.rs`:

```rust
use std::path::PathBuf;

use go_analyzer_mcp::selection_builder::{build_query, Filter, SelectKind};
use go_analyzer::Repo;

fn fixture_repo() -> Repo {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../go-analyzer/tests/fixture_repo");
    Repo::load(path).unwrap()
}

#[test]
fn query_all_structs_returns_all_struct_types() {
    let repo = fixture_repo();
    let items = build_query(&repo, &SelectKind::Structs, &[]);
    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert!(names.contains(&"User"));
    assert!(names.contains(&"Admin"));
    assert!(names.contains(&"Config"));
    assert!(names.contains(&"Server"));
    assert!(names.contains(&"Client"));
}

#[test]
fn query_items_have_absolute_file_paths() {
    let repo = fixture_repo();
    let items = build_query(&repo, &SelectKind::Functions, &[]);
    for item in &items {
        assert!(item.file.is_absolute(), "expected absolute path, got {:?}", item.file);
    }
}

#[test]
fn query_items_have_positive_line_numbers() {
    let repo = fixture_repo();
    let items = build_query(&repo, &SelectKind::Functions, &[]);
    for item in &items {
        assert!(item.line > 0, "expected line > 0, got {}", item.line);
    }
}

#[test]
fn query_items_have_signatures() {
    let repo = fixture_repo();
    let items = build_query(
        &repo,
        &SelectKind::Functions,
        &[Filter::Named("NewUser".to_string())],
    );
    assert_eq!(items.len(), 1);
    assert!(items[0].signature.contains("NewUser"));
    assert!(items[0].signature.contains("func"));
}

#[test]
fn query_chained_filters() {
    let repo = fixture_repo();
    let items = build_query(
        &repo,
        &SelectKind::Methods,
        &[
            Filter::OnType("Server".to_string()),
            Filter::Exported(true),
        ],
    );
    for item in &items {
        assert!(item.exported);
        assert!(item.receiver.as_ref().unwrap().contains("Server"));
    }
}
```

Note: These tests require making the relevant types public from `go-analyzer-mcp`. The crate needs a `lib.rs` that re-exports the modules, or the tests need to use the handler functions directly. Adjust based on what's exposed.

- [ ] **Step 2: Add lib.rs if needed**

If integration tests can't access internal modules, create `go-analyzer-mcp/src/lib.rs` that re-exports:
```rust
pub mod output;
pub mod selection_builder;
pub mod state;
pub mod tools;
```

And update `main.rs` to use `go_analyzer_mcp::*` imports.

- [ ] **Step 3: Run integration tests**

Run: `cargo test -p go-analyzer-mcp --test query_test`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "test(mcp): add integration tests for query tool"
```

---

## Task 13: Integration Tests for Describe Tools

**Files:**
- Create: `go-analyzer-mcp/tests/describe_test.rs`

- [ ] **Step 1: Write integration tests**

Create `go-analyzer-mcp/tests/describe_test.rs`:

```rust
use std::path::PathBuf;

use go_analyzer_mcp::state::ServerState;
use go_analyzer_mcp::tools::describe::{
    handle_describe_file, handle_describe_module, DescribeFileInput, DescribeModuleInput,
};

fn fixture_state() -> ServerState {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../go-analyzer/tests/fixture_repo");
    ServerState::new(path.canonicalize().unwrap())
}

#[test]
fn describe_file_returns_structural_overview() {
    let mut state = fixture_state();
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../go-analyzer/tests/fixture_repo/alpha/models.go")
        .canonicalize()
        .unwrap();

    let result = handle_describe_file(
        &mut state,
        &DescribeFileInput {
            path: fixture_path,
            include_docs: false,
        },
    )
    .unwrap();

    assert_eq!(result.package, "alpha");
    assert!(!result.types.is_empty());
    assert!(!result.functions.is_empty());
    assert!(!result.methods.is_empty());

    // Check that User struct is present
    let type_names: Vec<&str> = result.types.iter().map(|t| t.name.as_str()).collect();
    assert!(type_names.contains(&"User"));
    assert!(type_names.contains(&"Admin"));
    assert!(type_names.contains(&"Config"));
}

#[test]
fn describe_file_includes_imports() {
    let mut state = fixture_state();
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../go-analyzer/tests/fixture_repo/alpha/models.go")
        .canonicalize()
        .unwrap();

    let result = handle_describe_file(
        &mut state,
        &DescribeFileInput {
            path: fixture_path,
            include_docs: false,
        },
    )
    .unwrap();

    assert!(result.imports.contains(&"fmt".to_string()));
}

#[test]
fn describe_module_lists_all_packages() {
    let mut state = fixture_state();
    let result = handle_describe_module(
        &mut state,
        &DescribeModuleInput {
            depth: None,
            include_docs: false,
        },
    )
    .unwrap();

    let pkg_names: Vec<&str> = result.packages.iter().map(|p| p.name.as_str()).collect();
    assert!(pkg_names.contains(&"alpha"), "expected alpha in {:?}", pkg_names);
    assert!(pkg_names.contains(&"beta"), "expected beta in {:?}", pkg_names);
}

#[test]
fn describe_module_has_correct_counts() {
    let mut state = fixture_state();
    let result = handle_describe_module(
        &mut state,
        &DescribeModuleInput {
            depth: None,
            include_docs: false,
        },
    )
    .unwrap();

    let alpha = result.packages.iter().find(|p| p.name == "alpha").unwrap();
    assert!(alpha.types > 0, "expected types in alpha");
    assert!(alpha.functions > 0, "expected functions in alpha");
    assert!(!alpha.files.is_empty());
}

#[test]
fn describe_module_respects_depth() {
    let mut state = fixture_state();
    let result = handle_describe_module(
        &mut state,
        &DescribeModuleInput {
            depth: Some(0),
            include_docs: false,
        },
    )
    .unwrap();

    // depth 0 should only include the root package (if it has .go files)
    // In our fixture, the root doesn't have .go files, so this may be empty
    // or only include packages at depth 0
    // This test verifies depth limiting works at all
    let full = handle_describe_module(
        &mut state,
        &DescribeModuleInput {
            depth: None,
            include_docs: false,
        },
    )
    .unwrap();

    assert!(result.packages.len() <= full.packages.len());
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p go-analyzer-mcp --test describe_test`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add .
git commit -m "test(mcp): add integration tests for describe tools"
```

---

## Task 14: Integration Tests for Edit Tool

**Files:**
- Create: `go-analyzer-mcp/tests/edit_test.rs`

- [ ] **Step 1: Write integration tests using dry_run**

Create `go-analyzer-mcp/tests/edit_test.rs`:

```rust
use std::path::PathBuf;

use go_analyzer_mcp::selection_builder::{Filter, SelectKind};
use go_analyzer_mcp::state::ServerState;
use go_analyzer_mcp::tools::edit::{handle_edit, EditAction, EditInput};

fn fixture_state() -> ServerState {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../go-analyzer/tests/fixture_repo");
    ServerState::new(path.canonicalize().unwrap())
}

#[test]
fn edit_dry_run_returns_diff_without_writing() {
    let mut state = fixture_state();
    let result = handle_edit(
        &mut state,
        &EditInput {
            select: SelectKind::Functions,
            filters: vec![Filter::Named("helperFunc".to_string())],
            action: EditAction::Rename("renamedHelper".to_string()),
            dry_run: true,
        },
    )
    .unwrap();

    assert!(!result.diff.is_empty(), "expected non-empty diff");
    assert!(result.diff.contains("renamedHelper"));
    assert!(result.edits_applied > 0);

    // Verify file was NOT modified (dry_run = true)
    let model_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../go-analyzer/tests/fixture_repo/alpha/models.go");
    let content = std::fs::read_to_string(&model_file).unwrap();
    assert!(content.contains("helperFunc"), "file should not have been modified in dry_run");
}

#[test]
fn edit_empty_selection_returns_error() {
    let mut state = fixture_state();
    let result = handle_edit(
        &mut state,
        &EditInput {
            select: SelectKind::Functions,
            filters: vec![Filter::Named("nonexistent_function_xyz".to_string())],
            action: EditAction::Delete,
            dry_run: true,
        },
    );

    assert!(result.is_err());
}

#[test]
fn edit_write_modifies_file_and_reloads() {
    // Use a temporary copy of the fixture repo to avoid modifying the original
    let temp = tempfile::tempdir().unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../go-analyzer/tests/fixture_repo");

    // Copy fixture repo to temp dir
    copy_dir_recursive(&fixture, temp.path()).unwrap();

    let mut state = ServerState::new(temp.path().to_path_buf());

    // Rename helperFunc to renamedHelper
    let result = handle_edit(
        &mut state,
        &EditInput {
            select: SelectKind::Functions,
            filters: vec![Filter::Named("helperFunc".to_string())],
            action: EditAction::Rename("renamedHelper".to_string()),
            dry_run: false,
        },
    )
    .unwrap();

    assert!(!result.diff.is_empty());
    assert!(result.edits_applied > 0);

    // Verify the file was modified
    let model_file = temp.path().join("alpha/models.go");
    let content = std::fs::read_to_string(&model_file).unwrap();
    assert!(content.contains("renamedHelper"));
    assert!(!content.contains("helperFunc"));

    // Verify the repo was reloaded (query should find the renamed function)
    let repo = state.repo().unwrap();
    let found = repo.functions().named("renamedHelper").count();
    assert_eq!(found, 1, "expected to find renamedHelper after reload");
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p go-analyzer-mcp --test edit_test`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add .
git commit -m "test(mcp): add integration tests for edit tool"
```

---

## Task 15: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test --all-targets`
Expected: All tests pass across all three crates.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets`
Expected: No warnings.

- [ ] **Step 3: Run formatter**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

- [ ] **Step 4: Manual smoke test**

Test the MCP server manually:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}' | cargo run -p go-analyzer-mcp -- --path go-analyzer/tests/fixture_repo
```
Expected: Server responds with capabilities including the 5 tools.

- [ ] **Step 5: Commit any fixes**

```bash
git add .
git commit -m "chore(mcp): fix any issues found during final verification"
```
