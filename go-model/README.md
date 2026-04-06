# go-model

Pure data types representing the complete Go grammar as Rust types.

`go-model` provides a 1:1 structural mapping of every Go language construct -- expressions, statements, declarations, type expressions, and leaf literals -- as strongly-typed Rust enums and structs. All types derive `Serialize` and `Deserialize` via serde.

This crate contains no parsing logic, no tree-sitter dependency, and no I/O. It is the shared vocabulary between the parser (`go-analyzer`'s walker) and any tool that needs to inspect or generate Go ASTs.

## Key types

- [`SourceFile`] -- top-level: package declaration, imports, and declarations
- [`TopLevelDecl`] -- functions, methods, type definitions, vars, consts
- [`TypeExpr`] -- every Go type expression (named, pointer, slice, map, channel, func, interface, struct, generics)
- [`Expr`] -- every Go expression (identifiers, literals, calls, selectors, binary/unary ops, etc.)
- [`Stmt`] -- every Go statement (if, for, switch, select, return, assign, etc.)
- [`Span`] -- source location for every node; `Span::synthetic()` marks generated nodes

## `build` module

The `build` module provides ergonomic constructors for generating AST nodes programmatically:

```rust
use go_model::build;

let method = build::method(
    build::pointer_receiver("x", "MyStruct"),
    "String",
    vec![],
    vec![build::unnamed_param(build::named("string"))],
    build::block(vec![build::ret(vec![build::call(
        build::selector(build::ident("fmt"), "Sprintf"),
        vec![build::string("%+v"), build::deref(build::ident("x"))],
    )])]),
);
```

## License

Licensed under [Apache License, Version 2.0](../LICENSE).
