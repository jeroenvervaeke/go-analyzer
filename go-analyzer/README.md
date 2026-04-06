# go-analyzer

Analyze and transform Go source code from Rust with a fluent, type-safe API.

`go-analyzer` parses Go repositories into a complete typed AST ([`go-model`](../go-model/)), then provides a pipeline for querying, rewriting, and committing changes back to disk. The core workflow is:

```
Repo::load(".")  ->  Selection<T>  ->  query / Changes  ->  Applied  ->  commit
```

## Features

- **Fluent selection API** -- filter by name, export status, package, method presence
- **Pure changes** -- `Changes` is data, not side effects; nothing touches disk until `.commit()`
- **Call graph analysis** -- build a full symbol table with call edges, compute reachability, find dead code
- **Code generation** -- build Go AST nodes with the `build` module, insert them via `or_add`
- **Diff preview** -- inspect unified diffs before committing with `.preview()`

## Quick example

```rust
use go_analyzer::{Repo, build};

let repo = Repo::load(".")?;

// Add String() to exported structs that don't have one
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

repo.apply(changes).preview().commit()?;
```

See the `examples/` directory for more: querying repos, deleting methods, combining changes, and dead code elimination.

## License

Licensed under [Apache License, Version 2.0](../LICENSE).
