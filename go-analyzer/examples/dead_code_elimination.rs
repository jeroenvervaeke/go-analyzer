//! Dead code elimination for a Go repository.
//!
//! Builds a complete call/reference graph from the entry point(s),
//! finds all unreachable symbols (types, interfaces, methods, functions,
//! consts, vars), and removes them.
//!
//! Usage:
//!   cargo run --example dead_code_elimination -- <path-to-go-repo> [--entry <pkg/func>] [--dry-run]
//!
//! Examples:
//!   # Analyze mongodb-atlas-cli from main
//!   cargo run --example dead_code_elimination -- ~/git/github.com/mongodb/mongodb-atlas-cli --dry-run
//!
//!   # Analyze with a specific entry point
//!   cargo run --example dead_code_elimination -- ./myrepo --entry main.main

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use go_analyzer::Repo;
use go_analyzer::callgraph::{CallGraph, Symbol, SymbolKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let mut path = ".";
    let mut entry_spec: Option<&str> = None;
    let mut dry_run = false;
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--entry" => {
                i += 1;
                entry_spec = Some(&args[i]);
            }
            "--dry-run" => dry_run = true,
            "--verbose" | "-v" => verbose = true,
            other => path = other,
        }
        i += 1;
    }

    eprintln!("Loading repository from {}...", path);
    let repo = Repo::load(path)?;

    let file_count = repo.file_count();
    eprintln!("Loaded {file_count} Go files.");

    eprintln!("Building call graph...");
    let graph = CallGraph::build(&repo);
    eprintln!(
        "Symbol table: {} symbols, {} edges.",
        graph.symbols.len(),
        graph.edges.len()
    );

    // Find entry points
    let entries = find_entry_points(&graph, entry_spec);
    if entries.is_empty() {
        eprintln!("No entry points found. Use --entry <pkg.func> to specify one.");
        return Ok(());
    }
    eprintln!(
        "Entry points: {}",
        entries
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Compute reachability
    let reachable = graph.reachable_from(&entries);
    eprintln!("Reachable symbols: {}", reachable.len());

    // Find unreachable symbols (exclude builtins, test files, init functions)
    let unreachable: Vec<_> = graph
        .symbols
        .values()
        .filter(|entry| !reachable.contains(&entry.symbol))
        .filter(|entry| !is_test_file(&entry.file))
        .filter(|entry| !is_builtin_or_init(&entry.symbol.name))
        .collect();

    if unreachable.is_empty() {
        eprintln!("No dead code found!");
        return Ok(());
    }

    // Group by kind for summary
    let mut by_kind: HashMap<&str, Vec<_>> = HashMap::new();
    for entry in &unreachable {
        let kind = match &entry.kind {
            SymbolKind::Func => "functions",
            SymbolKind::Method { .. } => "methods",
            SymbolKind::Type => "types",
            SymbolKind::Var => "vars",
            SymbolKind::Const => "consts",
        };
        by_kind.entry(kind).or_default().push(entry);
    }

    eprintln!("\n=== Dead Code Summary ===");
    for (kind, entries) in &by_kind {
        eprintln!("  {}: {}", kind, entries.len());
    }
    eprintln!("  Total: {} unreachable symbols", unreachable.len());

    if verbose {
        eprintln!("\n=== Dead Symbols ===");
        for (kind, entries) in &by_kind {
            eprintln!("\n--- {kind} ---");
            for entry in entries {
                eprintln!(
                    "  {} ({}:{})",
                    entry.symbol.name,
                    entry.file.display(),
                    entry.span.start_row + 1,
                );
            }
        }
    }

    // Group unreachable symbols by file for deletion
    let mut deletions_by_file: HashMap<PathBuf, Vec<go_model::Span>> = HashMap::new();
    for entry in &unreachable {
        if !entry.span.is_synthetic() {
            deletions_by_file
                .entry(entry.file.clone())
                .or_default()
                .push(entry.span);
        }
    }

    let files_affected = deletions_by_file.len();
    let total_deletions = unreachable.len();

    eprintln!("\n{total_deletions} deletions across {files_affected} files.");

    if dry_run {
        eprintln!("(dry run — no files modified)");

        // Show a preview of what would be deleted per file
        let mut files: Vec<_> = deletions_by_file.keys().collect();
        files.sort();
        for file in files.iter().take(20) {
            let spans = &deletions_by_file[*file];
            eprintln!(
                "  {} ({} deletion{})",
                file.display(),
                spans.len(),
                if spans.len() == 1 { "" } else { "s" }
            );
        }
        if files.len() > 20 {
            eprintln!("  ... and {} more files", files.len() - 20);
        }
    } else {
        // Build Changes from the unreachable spans
        let edits: Vec<go_analyzer::edit::Edit> = unreachable
            .iter()
            .filter(|entry| !entry.span.is_synthetic())
            .map(|entry| go_analyzer::edit::Edit {
                file: entry.file.clone(),
                kind: go_analyzer::edit::EditKind::Delete { span: entry.span },
            })
            .collect();

        let changes = go_analyzer::Changes::from_edits(edits);
        let applied = repo.apply(changes);

        eprintln!(
            "Applying {} edits across {} files...",
            applied.edit_count(),
            applied.affected_files().len()
        );

        match applied.commit() {
            Ok(summary) => {
                eprintln!(
                    "Done: {} edits applied across {} files.",
                    summary.edits_applied, summary.files_modified
                );
            }
            Err(e) => {
                eprintln!("Error during commit: {e}");
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

/// Find entry points for reachability analysis.
fn find_entry_points(graph: &CallGraph, entry_spec: Option<&str>) -> Vec<Symbol> {
    if let Some(spec) = entry_spec {
        // User-specified entry point like "main.main"
        let parts: Vec<&str> = spec.splitn(2, '.').collect();
        if parts.len() == 2 {
            // Find the package directory that matches the package name
            let _pkg_name = parts[0];
            let func_name = parts[1];
            return graph
                .symbols
                .values()
                .filter(|entry| entry.symbol.name == func_name && entry.kind == SymbolKind::Func)
                .filter(|entry| {
                    // Check if this file's package matches
                    graph
                        .symbols
                        .values()
                        .any(|s| s.file == entry.file && s.symbol.pkg_dir == entry.symbol.pkg_dir)
                })
                .map(|entry| entry.symbol.clone())
                .collect();
        }
    }

    // Default: find all main() functions in package main
    let mut entries = Vec::new();

    for (sym, entry) in &graph.symbols {
        if sym.name == "main" && entry.kind == SymbolKind::Func {
            entries.push(sym.clone());
        }
        // Keep all exported symbols as entry points
        // (they could be used by external packages or via reflection)
        if entry.exported {
            entries.push(sym.clone());
        }
    }

    entries
}

fn is_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with("_test.go"))
}

fn is_builtin_or_init(name: &str) -> bool {
    matches!(name, "init" | "<init>" | "main" | "_")
}
