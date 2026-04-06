# go-analyzer

A Rust library for analyzing and transforming Go source code with a fluent, type-safe API.

[![CI](https://github.com/jeroenvervaeke/go-analyzer/actions/workflows/ci.yml/badge.svg)](https://github.com/jeroenvervaeke/go-analyzer/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

go-analyzer parses Go repositories into a complete, typed AST, then lets you query and rewrite them programmatically. No string manipulation, no regexes, no shelling out to `go` tooling. Everything stays in Rust types until you choose to commit changes to disk.

**Use cases**: large-scale Go refactoring, code generation, lint enforcement, dead code elimination, codebase analysis, automated migrations.

## Architecture

The project is a two-crate workspace:

| Crate | Purpose |
|---|---|
| **`go-model`** | Pure data types representing the full Go grammar. 1:1 structural mapping, fully serializable with serde, zero logic, zero dependencies on tree-sitter. |
| **`go-analyzer`** | Walker (tree-sitter to model), printer (model to source), resolver (cross-file imports), edit engine, call graph analysis, and the fluent query/change API. |

## Quick start

Add `go-analyzer` to your `Cargo.toml`:

```toml
[dependencies]
go-analyzer = { git = "https://github.com/jeroenvervaeke/go-analyzer" }
```

## Usage

### Query: list structs and count methods

```rust
use go_analyzer::Repo;

let repo = Repo::load("path/to/go/repo")?;

// List all structs
repo.structs().for_each(|t| {
    println!("{}", t.name().name);
});

// Count methods per struct
for si in repo.structs().collect() {
    let name = &si.item.name().name;
    let count = repo.methods().on_type(name).count();
    println!("{name}: {count} method(s)");
}

// Find exported structs missing String()
let missing = repo.structs().exported().method("String").absent().count();
println!("{missing} exported structs without String()");
```

### Change: add String() methods where missing

```rust
use go_analyzer::{Repo, build};

let repo = Repo::load("path/to/go/repo")?;

let changes = repo.structs().exported().method("String").or_add(|ts| {
    let name = &ts.name().name;
    build::method(
        build::pointer_receiver("x", name),
        "String",
        vec![],
        vec![build::unnamed_param(build::named("string"))],
        build::block(vec![build::ret(vec![build::call(
            build::selector(build::ident("fmt"), "Sprintf"),
            vec![build::string("%+v"), build::deref(build::ident("x"))],
        )])]),
    )
});

// Preview the diff without writing
repo.apply(changes).preview().commit()?;
```

### Combine multiple changes in one commit

```rust
use go_analyzer::{Changes, Repo};

let repo = Repo::load("path/to/go/repo")?;

let c1 = repo.structs().method("String").delete();
let c2 = repo.functions().named("OldName").rename("NewName");
let c3 = repo.types().named("LegacyClient").delete();

repo.apply(Changes::combine([c1, c2, c3]))
    .preview()
    .commit()?;
```

### Call graph: dead code elimination

```rust
use go_analyzer::Repo;
use go_analyzer::callgraph::{CallGraph, Symbol, SymbolKind};

let repo = Repo::load("path/to/go/repo")?;
let mut graph = CallGraph::build(&repo);

// Use all main() functions and exported symbols as entry points
let entries: Vec<Symbol> = graph.symbols.iter()
    .filter(|(_, e)| {
        (e.symbol.name == "main" && e.kind == SymbolKind::Func)
        || e.exported
    })
    .map(|(s, _)| s.clone())
    .collect();

// Iterative fixpoint: keeps pruning until no more dead code is found
let dead = graph.unreachable_fixpoint(&entries);
println!("{} unreachable symbols found", dead.len());
```

See [`go-analyzer/examples/`](go-analyzer/examples/) for complete, runnable versions of each use case — including [dead code elimination](go-analyzer/examples/dead_code_elimination.rs) tested against the MongoDB Atlas CLI.

### API at a glance

| Entry point | Returns | Description |
|---|---|---|
| `repo.functions()` | `Selection<FuncDecl>` | All top-level functions |
| `repo.methods()` | `Selection<MethodDecl>` | All methods |
| `repo.types()` / `.structs()` / `.interfaces()` | `Selection<TypeSpec>` | Type declarations |

| Filters | Terminals (query) | Terminals (change) |
|---|---|---|
| `.exported()` `.unexported()` | `.count()` `.collect()` | `.delete()` |
| `.named("X")` `.in_package("p")` | `.first()` `.is_empty()` | `.rename("Y")` |
| `.excluding_tests()` `.on_type("T")` | `.for_each(f)` | `.replace_body(f)` |
| `.method("M")` → `Selection<MethodEntry>` | `.existing()` `.absent()` | `.or_add(f)` `.and_modify(f)` |

## CLI

The `go-analyzer` binary provides common operations without writing Rust:

```bash
# List all structs
cargo run --bin go-analyzer -- --path ./my-go-repo structs

# List exported functions only
cargo run --bin go-analyzer -- --path ./my-go-repo --exported functions

# Add String() to all exported structs (dry run)
cargo run --bin go-analyzer -- --path ./my-go-repo --dry-run add-string-method

# Delete all methods named "Deprecated" (preview first)
cargo run --bin go-analyzer -- --path ./my-go-repo --dry-run delete-method Deprecated

# Filter to a specific package
cargo run --bin go-analyzer -- --path ./my-go-repo --package main functions
```

## How it works

```
source bytes (.go files)
    |
    v  walker  (tree-sitter-go -> treesitter-types-go CST -> go-model AST)
go-model types
    |
    |-> printer       go-model -> Go source string (internal, used by edit engine)
    |-> resolver      cross-file import + call resolution
    |-> callgraph     symbol table + call edges + reachability analysis
    '-> edit engine   Changes -> Applied -> commit to disk
             ^
        fluent API  (Repo -> Selection -> query terminals / Changes)
```

1. **Walk**: tree-sitter parses `.go` files into a CST (Concrete Syntax Tree — a lossless parse tree). The walker converts every CST node into a strongly-typed `go-model` AST. No information is lost — every Go construct has a corresponding Rust enum variant.
2. **Query or change**: the fluent `Selection<T>` API lets you filter by name, export status, package, method presence, etc. Terminal methods either return data (queries) or produce a `Changes` value (rewrites). Nothing touches disk until you call `.commit()`.
3. **Apply**: `Changes` describes edits as pure data. `Repo::apply(changes)` computes per-file diffs, producing an `Applied` that can be previewed or committed.
4. **Print**: the printer renders `go-model` types back to syntactically valid Go source, handling operator precedence, formatting, and all Go constructs.

## Testing

```bash
# Unit tests for both crates
cargo test --workspace

# Corpus test: roundtrip parse+print against the Go standard library ($GOROOT/src)
# Requires Go to be installed
cargo test -p go-analyzer --test corpus_test -- --nocapture
```

The corpus test parses every `.go` file in the Go standard library, converts it to `go-model`, prints it back to source, and verifies the output re-parses without errors. This covers thousands of files and exercises the full walker/printer pipeline.

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.

Copyright 2026 Jeroen Vervaeke
