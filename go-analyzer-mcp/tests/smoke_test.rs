//! Smoke test: exercise all 5 tools against the mongodb-atlas-cli repo.
//!
//! Set ATLAS_CLI_PATH env var to point to the repo, or skip these tests.

use std::path::PathBuf;

use go_analyzer_mcp::selection_builder::{Filter, SelectKind};
use go_analyzer_mcp::state::ServerState;
use go_analyzer_mcp::tools::call_graph::{CallGraphAction, CallGraphInput, handle_call_graph};
use go_analyzer_mcp::tools::describe::{
    DescribeFileInput, DescribeModuleInput, handle_describe_file, handle_describe_module,
};
use go_analyzer_mcp::tools::edit::{EditAction, EditInput, handle_edit};
use go_analyzer_mcp::tools::query::{QueryInput, handle_query};

fn atlas_cli_path() -> Option<PathBuf> {
    // Try well-known path first, then env var
    let well_known = PathBuf::from("/home/jeroen/git/github.com/mongodb/mongodb-atlas-cli");
    if well_known.exists() {
        return Some(well_known);
    }
    std::env::var("ATLAS_CLI_PATH").ok().map(PathBuf::from)
}

fn atlas_state() -> Option<ServerState> {
    atlas_cli_path().map(ServerState::new)
}

#[test]
fn smoke_describe_module() {
    let Some(mut state) = atlas_state() else {
        eprintln!("SKIP: atlas-cli not found");
        return;
    };

    // Use depth=3 — atlas-cli has packages nested at depth 2+ (e.g. cmd/atlas/, internal/cli/)
    let result = handle_describe_module(
        &mut state,
        DescribeModuleInput {
            depth: Some(3),
            include_docs: false,
        },
    )
    .unwrap();

    eprintln!("Module: {}", result.module);
    eprintln!("Packages (depth=3): {}", result.packages.len());
    assert!(
        !result.packages.is_empty(),
        "expected packages in atlas-cli"
    );

    for pkg in &result.packages {
        eprintln!(
            "  {} — {} types, {} funcs, {} methods, {} files",
            pkg.name,
            pkg.types,
            pkg.functions,
            pkg.methods,
            pkg.files.len()
        );
    }
}

#[test]
fn smoke_describe_module_full() {
    let Some(mut state) = atlas_state() else {
        return;
    };

    let result = handle_describe_module(
        &mut state,
        DescribeModuleInput {
            depth: None,
            include_docs: false,
        },
    )
    .unwrap();

    eprintln!(
        "Full module: {} packages, module={}",
        result.packages.len(),
        result.module
    );
    assert!(
        result.packages.len() > 10,
        "expected many packages, got {}",
        result.packages.len()
    );
}

#[test]
fn smoke_describe_file() {
    let Some(mut state) = atlas_state() else {
        return;
    };

    // Find a Go file to describe
    let path = atlas_cli_path().unwrap();
    let main_go = path.join("tools/cmd/docs/main.go");
    if !main_go.exists() {
        eprintln!("SKIP: tools/cmd/docs/main.go not found");
        return;
    }

    let result = handle_describe_file(
        &mut state,
        DescribeFileInput {
            path: main_go,
            include_docs: true,
        },
    )
    .unwrap();

    eprintln!("File package: {}", result.package);
    eprintln!("  imports: {}", result.imports.len());
    eprintln!("  types: {}", result.types.len());
    eprintln!("  functions: {}", result.functions.len());
    eprintln!("  methods: {}", result.methods.len());
    eprintln!("  constants: {}", result.constants.len());
    eprintln!("  variables: {}", result.variables.len());
}

#[test]
fn smoke_query_all_structs() {
    let Some(mut state) = atlas_state() else {
        return;
    };

    let result = handle_query(
        &mut state,
        &QueryInput {
            select: SelectKind::Structs,
            filters: vec![Filter::Exported(true)],
        },
    )
    .unwrap();

    eprintln!("Exported structs: {}", result.count);
    assert!(
        result.count > 50,
        "expected many exported structs, got {}",
        result.count
    );

    // Check output quality: every item has file, line, signature
    for item in &result.items {
        assert!(
            item.file.is_absolute(),
            "non-absolute path: {:?}",
            item.file
        );
        assert!(item.line > 0, "zero line for {}", item.name);
        assert!(!item.signature.is_empty(), "empty sig for {}", item.name);
    }

    // Print first 10
    for item in result.items.iter().take(10) {
        eprintln!(
            "  {} ({}:{}) — {}",
            item.name,
            item.file.display(),
            item.line,
            item.signature
        );
    }
}

#[test]
fn smoke_query_methods_on_type() {
    let Some(mut state) = atlas_state() else {
        return;
    };

    // Try to find methods - query all exported methods
    let result = handle_query(
        &mut state,
        &QueryInput {
            select: SelectKind::Methods,
            filters: vec![Filter::Exported(true)],
        },
    )
    .unwrap();

    eprintln!("Exported methods: {}", result.count);
    assert!(
        result.count > 100,
        "expected many methods, got {}",
        result.count
    );
}

#[test]
fn smoke_query_interfaces() {
    let Some(mut state) = atlas_state() else {
        return;
    };

    let result = handle_query(
        &mut state,
        &QueryInput {
            select: SelectKind::Interfaces,
            filters: vec![Filter::Exported(true)],
        },
    )
    .unwrap();

    eprintln!("Exported interfaces: {}", result.count);
    for item in result.items.iter().take(10) {
        eprintln!("  {} ({}:{})", item.name, item.file.display(), item.line);
    }
}

#[test]
fn smoke_call_graph_dead_code() {
    let Some(mut state) = atlas_state() else {
        return;
    };

    let result = handle_call_graph(
        &mut state,
        &CallGraphInput {
            action: CallGraphAction::DeadCode,
            symbol: None,
        },
    )
    .unwrap();

    eprintln!("Dead code symbols: {}", result.nodes.len());
    for node in result.nodes.iter().take(10) {
        eprintln!("  {} ({}:{})", node.symbol, node.file.display(), node.line);
    }
}

#[test]
fn smoke_edit_dry_run() {
    let Some(mut state) = atlas_state() else {
        return;
    };

    // Find a function and try a dry-run rename
    let query = handle_query(
        &mut state,
        &QueryInput {
            select: SelectKind::Functions,
            filters: vec![Filter::Exported(false), Filter::ExcludingTests(true)],
        },
    )
    .unwrap();

    if query.items.is_empty() {
        eprintln!("SKIP: no unexported non-test functions found");
        return;
    }

    let target = &query.items[0];
    eprintln!(
        "Dry-run rename of {} ({}:{})",
        target.name,
        target.file.display(),
        target.line
    );

    let result = handle_edit(
        &mut state,
        &EditInput {
            select: SelectKind::Functions,
            filters: vec![Filter::Named(target.name.clone())],
            action: EditAction::Rename("renamedBySmoke".to_string()),
            dry_run: true,
        },
    )
    .unwrap();

    eprintln!(
        "Diff ({} edits):\n{}",
        result.edits_applied,
        &result.diff[..result.diff.len().min(500)]
    );
    assert!(!result.diff.is_empty(), "expected non-empty diff");
    assert!(result.diff.contains("renamedBySmoke"));
}
