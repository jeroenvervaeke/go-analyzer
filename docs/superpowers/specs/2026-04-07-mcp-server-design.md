# go-analyzer MCP Server Design

## Overview

An MCP (Model Context Protocol) server that wraps the `go-analyzer` crate, enabling LLMs to query, analyze, and edit Go codebases through structured tool calls. The primary consumer is LLM agents (Claude, GPT, etc.) during coding tasks, with secondary use by developers via LLM-powered IDEs.

## Decisions

- **New workspace crate:** `go-analyzer-mcp`, lives in the existing workspace alongside `go-analyzer` and `go-model`.
- **Transport:** stdio only (via `rmcp` SDK). Universal baseline for local MCP tool use.
- **State:** Persistent `Repo` in memory. Lazy-loaded on first tool call, auto-reloaded after edits.
- **Repo path:** Defaults to working directory. Optional `--path` CLI flag for override.
- **Tool design:** Medium-grained. Each tool takes a pipeline of operations mirroring the fluent `Selection` API.
- **Edit behavior:** Auto-applies to disk by default, returns unified diff. `dry_run: true` to preview without writing.
- **Performance:** No special optimization for large repos. Target small-to-medium codebases initially.

## Tools

### 1. `query`

Find and filter Go declarations using a pipeline of select + filters.

**Input:**

```json
{
  "select": "methods",
  "filters": [
    {"on_type": "User"},
    {"exported": true}
  ]
}
```

**`select` values:** `functions`, `methods`, `structs`, `interfaces`, `types`

**`filters`** (chainable, applied in order):

| Filter              | Value    | Maps to                          |
|---------------------|----------|----------------------------------|
| `named`             | string   | `.named(name)`                   |
| `in_package`        | string   | `.in_package(name)`              |
| `exported`          | bool     | `.exported()` / `.unexported()`  |
| `excluding_tests`   | bool     | `.excluding_tests()`             |
| `on_type`           | string   | `.on_type(type_name)`            |
| `implementing`      | string   | `.implementing(interface_name)`  |

**Output:**

```json
{
  "items": [
    {
      "name": "HandleRequest",
      "kind": "method",
      "receiver": "*Server",
      "package": "server",
      "file": "/absolute/path/to/server.go",
      "line": 42,
      "end_line": 67,
      "exported": true,
      "signature": "func (s *Server) HandleRequest(ctx context.Context, req *Request) (*Response, error)"
    }
  ],
  "count": 1
}
```

Every item includes absolute file path, line numbers, and full signature. No ambiguity about location.

### 2. `call_graph`

Analyze call relationships between symbols.

**Input:**

```json
{
  "action": "callers",
  "symbol": "HandleRequest"
}
```

**`action` values:** `callers`, `callees`, `reachable_from`, `dead_code`

**Output:**

```json
{
  "graph": {
    "nodes": [
      {"symbol": "server.HandleRequest", "file": "/path/server.go", "line": 42}
    ],
    "edges": [
      {"from": "server.HandleRequest", "to": "auth.ValidateToken"}
    ]
  },
  "text": "server.HandleRequest\n  -> auth.ValidateToken (/path/auth.go:15)\n  -> db.QueryUser (/path/db.go:88)\n    -> db.connect (/path/db.go:12)\n"
}
```

Returns both structured graph data and a human/LLM-readable indented text tree.

### 3. `edit`

Modify Go code using the same select + filters pipeline, plus an action.

**Input:**

```json
{
  "select": "methods",
  "filters": [
    {"on_type": "User"},
    {"named": "String"}
  ],
  "action": {"replace_body": "return fmt.Sprintf(\"User(%s)\", u.Name)"},
  "dry_run": false
}
```

**`action` values:**

| Action                          | Maps to                              |
|---------------------------------|--------------------------------------|
| `{"delete": true}`              | `.delete()`                          |
| `{"rename": "NewName"}`         | `.rename("NewName")`                 |
| `{"replace_body": "..."}`       | `.replace_body(body)`                |
| `{"add_field": ["name", "ty"]}` | `.add_field("name", "ty")`           |
| `{"remove_field": "name"}`      | `.remove_field("name")`              |
| `{"add_method": {"name": "...", "receiver": "...", "params": [...], "returns": [...], "body": "..."}}`  | `.method("name").or_add(closure)` |
| `{"modify_method": {"name": "...", "body": "..."}}`  | `.method("name").and_modify(closure)` |

Note: The exact JSON schema for `add_method` and `modify_method` will be refined during implementation. These actions construct AST nodes via the `go-model` builder API — the JSON representation needs to balance expressiveness with simplicity for LLM callers.

**Behavior:**
- Default (`dry_run: false`): applies changes to disk, returns unified diff.
- `dry_run: true`: returns unified diff without writing.
- After a non-dry-run edit, the server auto-reloads the `Repo` so subsequent queries reflect the new state.
- If the selection matches nothing, returns an error with the filters echoed back so the LLM can adjust.

**Output:**

```json
{
  "diff": "--- a/server.go\n+++ b/server.go\n@@ -42,3 +42,3 @@\n...",
  "files_modified": ["/absolute/path/to/server.go"],
  "edits_applied": 1
}
```

### 4. `describe_file`

Structural overview of a single Go source file.

**Input:**

```json
{
  "path": "/path/to/server.go",
  "include_docs": true
}
```

**Output:**

```json
{
  "package": "server",
  "imports": ["context", "fmt", "net/http"],
  "types": [
    {
      "name": "Server",
      "kind": "struct",
      "line": 12,
      "exported": true,
      "doc": "Server handles incoming HTTP requests."
    }
  ],
  "functions": [
    {
      "name": "New",
      "line": 25,
      "signature": "func New(addr string) *Server",
      "exported": true,
      "doc": "New creates a Server with the given address."
    }
  ],
  "methods": [
    {
      "name": "HandleRequest",
      "receiver": "*Server",
      "line": 42,
      "signature": "func (s *Server) HandleRequest(...) (*Response, error)",
      "exported": true,
      "doc": null
    }
  ],
  "constants": [
    {"name": "MaxRetries", "line": 8, "value": "3", "exported": true}
  ],
  "variables": [
    {"name": "defaultTimeout", "line": 10, "exported": false}
  ]
}
```

With `include_docs: false`, `doc` fields are omitted.

### 5. `describe_module`

Package tree of the loaded Go module.

**Input:**

```json
{
  "depth": null,
  "include_docs": false
}
```

- `depth: null` — full tree. `depth: 1` — top-level packages only. Enables progressive exploration.
- `include_docs` — include package-level doc comments.

**Output:**

```json
{
  "module": "github.com/example/myservice",
  "path": "/absolute/path/to/myservice",
  "packages": [
    {
      "name": "main",
      "import_path": "github.com/example/myservice",
      "path": "/absolute/path/to/myservice",
      "files": ["main.go", "config.go"],
      "types": 2,
      "functions": 5,
      "methods": 8,
      "constants": 3,
      "doc": null
    },
    {
      "name": "auth",
      "import_path": "github.com/example/myservice/auth",
      "path": "/absolute/path/to/myservice/auth",
      "files": ["auth.go", "token.go", "middleware.go"],
      "types": 4,
      "functions": 3,
      "methods": 12,
      "constants": 1,
      "doc": "Package auth provides JWT-based authentication."
    }
  ]
}
```

## MCP Prompt

A single prompt to guide LLMs on the intended workflow:

**Name:** `go-analyzer`

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

## Architecture

### Crate Structure

```
go-analyzer-mcp/
  Cargo.toml          # depends on go-analyzer, go-model, rmcp, tokio, serde, serde_json
  src/
    main.rs           # entry point: parse CLI args, init state, start stdio transport
    server.rs         # MCP server struct, tool dispatch, holds Arc<Mutex<ServerState>>
    state.rs          # ServerState { repo, repo_path }
    tools/
      mod.rs          # shared types: SelectKind, Filter, output structs
      query.rs        # query tool handler
      call_graph.rs   # call_graph tool handler
      edit.rs         # edit tool handler
      describe.rs     # describe_file + describe_module handlers
```

### State

```rust
struct ServerState {
    repo: Option<Repo>,
    repo_path: PathBuf,
}
```

- `repo_path` set from `--path` CLI arg or current working directory.
- `repo` is `None` until first tool call triggers lazy load.
- After a non-dry-run `edit`, `repo` is reloaded from disk.

### Shared Selection Builder

`query` and `edit` share the same `select` + `filters` front-end:

```rust
fn build_selection(repo: &Repo, select: SelectKind, filters: &[Filter]) -> SelectionResult
```

This avoids duplicating filter logic between the two tools.

### Error Handling

- **Bad repo path / no `.go` files:** Clear error with the path included.
- **Empty selection on query:** Success with `count: 0`, empty items. Not an error.
- **Empty selection on edit:** Error: "no items matched, nothing to edit" with filters echoed back.
- **Parse errors in individual files:** Load succeeds, unparseable files skipped with warnings in the response.

## Testing Strategy

### Unit Tests

- `build_selection` correctly chains filters and returns expected items.
- Each filter works in isolation.
- Edit actions produce correct `Changes` for each operation type.
- Output serialization includes all location fields.
- `describe_file` and `describe_module` produce correct structural summaries.
- Call graph text rendering matches expected tree format.
- Error cases: empty selection edit returns error, missing repo returns clear message.

### Integration Tests

Against the existing fixture repo at `go-analyzer/tests/fixture_repo` (extended if needed for multi-package structure):

- Full tool round-trips: JSON input -> tool handler -> JSON output.
- `query` results match known fixture repo content.
- `edit` with `dry_run: true` returns diff, files unchanged.
- `edit` without dry_run returns diff, files changed, auto-reload produces updated query results.
- `describe_module` package tree matches fixture repo structure.
- `describe_file` structural overview matches known file contents.
- `call_graph` edges match known call relationships.

### Not Tested

The `rmcp` transport layer — that's the SDK's responsibility. Tool handlers are tested via direct function calls.

## Known Gaps

- **Doc comments:** The `go-model` AST may not currently capture doc comments (comments preceding declarations). If not, the `include_docs` feature in `describe_file` and `describe_module` will require adding comment extraction to the `walker`. This should be verified early in implementation and scoped as a separate task if needed.
- **`describe_file` path format:** Accepts absolute paths only. LLMs get absolute paths from `describe_module` and `query` output, so this should be sufficient. Relative path support can be added later if needed.
