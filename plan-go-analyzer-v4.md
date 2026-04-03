# go-analyzer — full plan

## Two crates

- **`go-model`** — complete 1:1 structural representation of the Go grammar.
  No tree-sitter dependency. No logic. Fully serialisable.
- **`go-analyzer`** — everything else: walker, resolver, printer, edit engine,
  and the fluent query/change API. Users import this one crate.

## User-facing flow

```
1. Load      Repo::load(".")
2. Build     repo.structs().method("String").or_add(|t| ...)  →  Changes
3. Apply     repo.apply(changes)                               →  Applied
4. Commit    applied.commit()
```

`Changes` is pure data — a description of edits with no side effects.
`Applied` holds the computed per-file diffs and writes them on `.commit()`.
Nothing touches disk until `.commit()` is called.

---

## Core design principle: query vs changes

**`Selection<T>` is neutral.** It is just a filtered view over the repo. It is
neither a query nor a change on its own. It becomes one or the other only when
you call a terminal method on it.

**The return type of the terminal method is the contract:**

| Terminal method | Returns | Meaning |
|---|---|---|
| `.collect()` | `Vec<T>` | read-only query |
| `.count()` | `usize` | read-only query |
| `.for_each(f)` | `()` | read-only query |
| `.is_empty()` | `bool` | read-only query |
| `.first()` | `Option<T>` | read-only query |
| `.delete()` | `Changes` | source will be modified |
| `.rename(f)` | `Changes` | source will be modified |
| `.replace_body(f)` | `Changes` | source will be modified |
| `.or_add(f)` | `Changes` | source will be modified |
| `.and_modify(f)` | `Changes` | source will be modified |
| `.add_field(f)` | `Changes` | source will be modified |
| `.remove_field(n)` | `Changes` | source will be modified |

**Implementation rule for Claude Code:** every method on `Selection<T>` or
`Selection<MethodEntry>` that returns `Changes` must be pure — it computes and
returns a `Changes` value but does NOT write anything, call `gofmt`, or touch
the filesystem. All side effects happen only inside `Applied::commit()`.
Methods returning data types (`Vec`, `usize`, `bool`, etc.) must also be pure
and must never produce a `Changes` as a side effect.

**The same selection can be used for both, independently:**

```rust
let sel = repo.structs().exported().method("String");

// Use it as a query — read only, no changes
let types_with_string = sel.existing().collect();
println!("{} structs already have String()", types_with_string.len());

// Use it as a change — produces Changes, nothing written yet
let changes = sel.or_add(|t| build::method(...));

// Only now does anything get written
repo.apply(changes).commit()?;
```

Note: `sel` above would need to be re-created each time since `Selection` is
consumed by terminal methods (it is not `Copy`). In practice users just chain:

```rust
// Query
let count = repo.structs().exported().method("String").existing().count();

// Change  
let changes = repo.structs().exported().method("String").or_add(|t| ...);
```

---

## Architecture (inside `go-analyzer`)

```
source bytes
    │
    ▼  walker  (uses treesitter-types-go)
go-model  ←────────────────────────────── also used for generation via build::*
    │
    ├──▶ printer   go-model → source string  (used internally by edit engine)
    ├──▶ resolver  cross-file call/import resolution
    └──▶ edit engine  Changes → Applied → commit to disk
              ▲
         fluent API  (Selection, MethodEntry, Changes)
```

---

## Crate 1 — `go-model`

Complete, 1:1 structural representation of the Go grammar. Every node that
tree-sitter-go parses has a corresponding Rust type here. No strings used where
structure is known. No tree-sitter dependency.

### Source location

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

impl Span {
    /// Sentinel value for generated nodes that have no source location.
    /// The printer ignores spans. The edit engine uses this to distinguish
    /// "replace existing source" (real span) from "insert new source" (synthetic).
    pub fn synthetic() -> Self {
        Self { start_byte: 0, end_byte: 0, start_row: 0, start_col: 0, end_row: 0, end_col: 0 }
    }

    pub fn is_synthetic(&self) -> bool {
        self.start_byte == 0 && self.end_byte == 0
    }
}
```

### Type expressions (`TypeExpr`)

Structured — no `type_repr: String`. Every Go type has a variant.

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeExpr {
    Named(Ident),
    Qualified { package: Ident, name: Ident },
    Pointer(Box<TypeExpr>),
    Array { len: Box<Expr>, elem: Box<TypeExpr> },
    Slice(Box<TypeExpr>),
    Map { key: Box<TypeExpr>, value: Box<TypeExpr> },
    Channel { direction: ChanDir, elem: Box<TypeExpr> },
    Func(FuncType),
    Interface(InterfaceType),
    Struct(StructType),
    Generic { base: Box<TypeExpr>, args: Vec<TypeExpr> },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ChanDir { Both, Recv, Send }
```

### Expressions (`Expr`)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Ident(Ident),
    Qualified { package: Ident, name: Ident, span: Span },
    Int(IntLit),
    Float(FloatLit),
    Imaginary(ImaginaryLit),
    Rune(RuneLit),
    String(StringLit),
    RawString(RawStringLit),
    True(Span),
    False(Span),
    Nil(Span),
    Iota(Span),
    Composite { ty: Box<TypeExpr>, elems: Vec<KeyedElem>, span: Span },
    FuncLit { ty: FuncType, body: Block, span: Span },
    Call {
        func: Box<Expr>,
        type_args: Vec<TypeExpr>,
        args: Vec<Expr>,
        ellipsis: bool,
        span: Span,
    },
    Selector { operand: Box<Expr>, field: Ident, span: Span },
    Index { operand: Box<Expr>, index: Box<Expr>, span: Span },
    Slice {
        operand: Box<Expr>,
        low: Option<Box<Expr>>,
        high: Option<Box<Expr>>,
        max: Option<Box<Expr>>,
        span: Span,
    },
    TypeAssert { operand: Box<Expr>, ty: Box<TypeExpr>, span: Span },
    Unary { op: UnaryOp, operand: Box<Expr>, span: Span },
    Binary { op: BinaryOp, left: Box<Expr>, right: Box<Expr>, span: Span },
    Paren(Box<Expr>, Span),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyedElem { pub key: Option<Expr>, pub value: Expr, pub span: Span }

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum UnaryOp { Not, Neg, Deref, Addr, Recv, BitNot }

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add, Sub, Mul, Div, Rem,
    And, Or, Xor, AndNot, Shl, Shr,
    LogAnd, LogOr,
    Eq, Ne, Lt, Le, Gt, Ge,
}
```

### Statements (`Stmt`)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    Block(Block),
    Expr(Expr, Span),
    Assign { lhs: Vec<Expr>, op: AssignOp, rhs: Vec<Expr>, span: Span },
    ShortVarDecl { names: Vec<Ident>, values: Vec<Expr>, span: Span },
    VarDecl(VarSpec, Span),
    ConstDecl(ConstSpec, Span),
    Inc(Expr, Span),
    Dec(Expr, Span),
    Send { channel: Expr, value: Expr, span: Span },
    Return { values: Vec<Expr>, span: Span },
    If {
        init: Option<Box<Stmt>>,
        cond: Expr,
        body: Block,
        else_: Option<Box<Stmt>>,
        span: Span,
    },
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        post: Option<Box<Stmt>>,
        body: Block,
        span: Span,
    },
    ForRange {
        key: Option<Expr>,
        value: Option<Expr>,
        assign: RangeAssign,
        iterable: Expr,
        body: Block,
        span: Span,
    },
    Switch {
        init: Option<Box<Stmt>>,
        tag: Option<Expr>,
        cases: Vec<ExprCase>,
        span: Span,
    },
    TypeSwitch {
        init: Option<Box<Stmt>>,
        assign: TypeSwitchAssign,
        cases: Vec<TypeCase>,
        span: Span,
    },
    Select { cases: Vec<CommCase>, span: Span },
    Go(Expr, Span),
    Defer(Expr, Span),
    Break(Option<Ident>, Span),
    Continue(Option<Ident>, Span),
    Goto(Ident, Span),
    Fallthrough(Span),
    Labeled { label: Ident, body: Box<Stmt>, span: Span },
    TypeDecl(TypeSpec, Span),
    Empty(Span),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AssignOp {
    Assign,
    AddAssign, SubAssign, MulAssign, DivAssign, RemAssign,
    AndAssign, OrAssign, XorAssign, AndNotAssign, ShlAssign, ShrAssign,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RangeAssign { Define, Assign }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block { pub stmts: Vec<Stmt>, pub span: Span }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExprCase { pub exprs: Vec<Expr>, pub body: Vec<Stmt>, pub span: Span }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeCase { pub types: Vec<TypeExpr>, pub body: Vec<Stmt>, pub span: Span }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeSwitchAssign { pub name: Option<Ident>, pub expr: Expr, pub span: Span }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommCase {
    Send { stmt: Stmt, body: Vec<Stmt>, span: Span },
    Recv { stmt: Option<Stmt>, body: Vec<Stmt>, span: Span },
    Default { body: Vec<Stmt>, span: Span },
}
```

### Declarations

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuncType {
    pub type_params: Vec<TypeParam>,
    pub params: Vec<ParamDecl>,
    pub results: Vec<ParamDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamDecl {
    pub names: Vec<Ident>,
    pub ty: TypeExpr,
    pub variadic: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeParam {
    pub names: Vec<Ident>,
    pub constraint: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receiver {
    pub name: Option<Ident>,
    pub type_params: Vec<TypeParam>,
    pub ty: TypeExpr,   // always Named or Pointer(Named)
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuncDecl {
    pub name: Ident,
    pub ty: FuncType,
    pub body: Option<Block>,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodDecl {
    pub receiver: Receiver,
    pub name: Ident,
    pub ty: FuncType,
    pub body: Option<Block>,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructType {
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldDecl {
    Named {
        names: Vec<Ident>,
        ty: TypeExpr,
        tag: Option<StringLit>,
        span: Span,
    },
    Embedded {
        ty: TypeExpr,
        tag: Option<StringLit>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceType {
    pub elements: Vec<InterfaceElem>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InterfaceElem {
    Method { name: Ident, ty: FuncType, span: Span },
    TypeTerm(TypeTerm),
    Embedded(TypeExpr),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeTerm {
    pub terms: Vec<TypeTermElem>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeTermElem {
    pub tilde: bool,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeSpec {
    Alias { name: Ident, type_params: Vec<TypeParam>, ty: TypeExpr, span: Span },
    Def   { name: Ident, type_params: Vec<TypeParam>, ty: TypeExpr, span: Span },
}

impl TypeSpec {
    pub fn name(&self) -> &Ident {
        match self { Self::Alias { name, .. } | Self::Def { name, .. } => name }
    }
    pub fn is_struct(&self) -> bool {
        matches!(self, Self::Def { ty: TypeExpr::Struct(_), .. })
    }
    pub fn is_interface(&self) -> bool {
        matches!(self, Self::Def { ty: TypeExpr::Interface(_), .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VarSpec {
    pub names: Vec<Ident>,
    pub ty: Option<TypeExpr>,
    pub values: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstSpec {
    pub names: Vec<Ident>,
    pub ty: Option<TypeExpr>,
    pub values: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSpec {
    pub alias: ImportAlias,
    pub path: StringLit,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImportAlias {
    Implicit,
    Dot,
    Blank,
    Named(Ident),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceFile {
    pub package: Ident,
    pub imports: Vec<ImportSpec>,
    pub decls: Vec<TopLevelDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TopLevelDecl {
    Func(FuncDecl),
    Method(MethodDecl),
    Type(Vec<TypeSpec>),
    Var(Vec<VarSpec>),
    Const(Vec<ConstSpec>),
}
```

### Leaf types

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    pub fn synthetic(name: &str) -> Self {
        Self { name: name.to_owned(), span: Span::synthetic() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StringLit { pub raw: String, pub span: Span }

impl StringLit {
    pub fn value(&self) -> String { /* strip quotes + unescape */ todo!() }
    pub fn from_value(s: &str) -> Self {
        Self { raw: format!("\"{}\"", s.escape_default()), span: Span::synthetic() }
    }
}

// IntLit, FloatLit, ImaginaryLit, RuneLit, RawStringLit follow the same pattern:
// raw: String (original source text), span: Span
```

### `build` module — constructors for generation

```rust
pub mod build {
    use super::*;

    // --- types ---
    pub fn named(name: &str) -> TypeExpr {
        TypeExpr::Named(Ident::synthetic(name))
    }
    pub fn pointer(inner: TypeExpr) -> TypeExpr {
        TypeExpr::Pointer(Box::new(inner))
    }
    pub fn slice(elem: TypeExpr) -> TypeExpr {
        TypeExpr::Slice(Box::new(elem))
    }
    pub fn map_type(key: TypeExpr, value: TypeExpr) -> TypeExpr {
        TypeExpr::Map { key: Box::new(key), value: Box::new(value) }
    }

    // --- exprs ---
    pub fn ident(name: &str) -> Expr {
        Expr::Ident(Ident::synthetic(name))
    }
    pub fn string(value: &str) -> Expr {
        Expr::String(StringLit::from_value(value))
    }
    pub fn int(value: i64) -> Expr {
        Expr::Int(IntLit { raw: value.to_string(), span: Span::synthetic() })
    }
    pub fn call(func: Expr, args: Vec<Expr>) -> Expr {
        Expr::Call {
            func: Box::new(func),
            type_args: vec![],
            args,
            ellipsis: false,
            span: Span::synthetic(),
        }
    }
    pub fn selector(operand: Expr, field: &str) -> Expr {
        Expr::Selector {
            operand: Box::new(operand),
            field: Ident::synthetic(field),
            span: Span::synthetic(),
        }
    }
    pub fn deref(operand: Expr) -> Expr {
        Expr::Unary { op: UnaryOp::Deref, operand: Box::new(operand), span: Span::synthetic() }
    }
    pub fn addr(operand: Expr) -> Expr {
        Expr::Unary { op: UnaryOp::Addr, operand: Box::new(operand), span: Span::synthetic() }
    }

    // --- stmts ---
    pub fn ret(values: Vec<Expr>) -> Stmt {
        Stmt::Return { values, span: Span::synthetic() }
    }
    pub fn block(stmts: Vec<Stmt>) -> Block {
        Block { stmts, span: Span::synthetic() }
    }

    // --- declarations ---
    pub fn param(names: &[&str], ty: TypeExpr) -> ParamDecl {
        ParamDecl {
            names: names.iter().map(|n| Ident::synthetic(n)).collect(),
            ty,
            variadic: false,
            span: Span::synthetic(),
        }
    }
    pub fn unnamed_param(ty: TypeExpr) -> ParamDecl {
        ParamDecl { names: vec![], ty, variadic: false, span: Span::synthetic() }
    }
    pub fn pointer_receiver(var_name: &str, type_name: &str) -> Receiver {
        Receiver {
            name: Some(Ident::synthetic(var_name)),
            type_params: vec![],
            ty: pointer(named(type_name)),
            span: Span::synthetic(),
        }
    }
    pub fn method(
        receiver: Receiver,
        name: &str,
        params: Vec<ParamDecl>,
        results: Vec<ParamDecl>,
        body: Block,
    ) -> MethodDecl {
        MethodDecl {
            receiver,
            name: Ident::synthetic(name),
            ty: FuncType {
                type_params: vec![],
                params,
                results,
                span: Span::synthetic(),
            },
            body: Some(body),
            doc: None,
            span: Span::synthetic(),
        }
    }
}
```

---

## Crate 2 — `go-analyzer`

### Internal modules

```
go-analyzer/src/
  lib.rs          re-exports Repo, Selection, Changes, Applied, build (from go-model)
  walker.rs       treesitter-types-go CST → SourceFile (go-model)
  resolver.rs     cross-file import + call resolution
  printer.rs      go-model → source string + gofmt post-pass
  edit.rs         Edit, apply edits to source bytes
  repo.rs         Repo struct, load_repo, file storage
  selection.rs    Selection<T>, MethodEntry
  changes.rs      Changes, Changes::combine
  applied.rs      Applied, commit
```

### `Repo`

```rust
pub struct Repo { /* internal */ }

impl Repo {
    pub fn load(path: impl AsRef<Path>) -> Result<Self>;

    // Selection entry points
    pub fn functions(&self) -> Selection<'_, FuncDecl>;
    pub fn methods(&self)   -> Selection<'_, MethodDecl>;
    pub fn types(&self)     -> Selection<'_, TypeSpec>;
    pub fn structs(&self)   -> Selection<'_, TypeSpec>;    // sugar: types().structs()
    pub fn interfaces(&self)-> Selection<'_, TypeSpec>;    // sugar: types().interfaces()

    // Apply changes → Applied
    pub fn apply(&self, changes: Changes) -> Applied<'_>;
}
```

### Query vs Changes — the core contract

**This is the most important design rule in the entire codebase. Every method
on `Selection` is either a query terminal or a change terminal. There is no
third category.**

A `Selection<T>` is just a filtered view — it is neither a query nor a change
until you call a terminal method on it. The terminal method name is the sole
signal to the caller:

| Terminal returns | Meaning | Examples |
|---|---|---|
| `T`, `Vec<T>`, `usize`, `bool` | **Query** — reads data, no side effects | `.collect()`, `.count()`, `.first()`, `.is_empty()` |
| `Changes` | **Change** — describes source edits, no side effects | `.delete()`, `.rename(...)`, `.or_add(...)`, `.replace_body(...)` |

Both kinds are pure and side-effect-free. Neither touches disk. The only thing
that touches disk is `Applied::commit()`.

```rust
// The SAME selection terminated two different ways:
let sel = repo.structs().exported().method("String");

// Query — returns data, nothing changes
let n = sel.existing().count();

// Change — returns Changes, nothing changes yet
let changes = sel.delete();
// still nothing has changed on disk at this point

// Only now does anything get written
repo.apply(changes).commit()?;
```

**Implementation rule**: every method on `Selection<T>` and `Selection<MethodEntry>`
must return exactly one of: `Self`, `Selection<U>`, a plain data type, or `Changes`.
No method may perform I/O, mutate the `Repo`, or produce `Applied` directly.
`Applied` is only ever produced by `Repo::apply(changes)`.

### `Selection<T>`

Eager `Vec<T>` internally. Filters call `.retain()`. Cheap to reason about,
easy to debug (`.count()` at any point in the chain).

```rust
pub struct Selection<'repo, T> {
    repo: &'repo Repo,
    items: Vec<T>,
}

// Universal
impl<'repo, T> Selection<'repo, T> {
    pub fn filter(mut self, pred: impl Fn(&T) -> bool) -> Self;
    pub fn count(&self) -> usize;
    pub fn collect(self) -> Vec<T>;
    pub fn for_each(self, f: impl Fn(&T));
    pub fn is_empty(&self) -> bool;
    pub fn in_package(self, pkg: &str) -> Self;
    pub fn exported(self) -> Self;
    pub fn unexported(self) -> Self;
    pub fn excluding_tests(self) -> Self;
}

// FuncDecl
impl<'repo> Selection<'repo, FuncDecl> {
    pub fn named(self, name: &str) -> Self;
    pub fn calling(self, fqn: &str) -> Self;
    pub fn unreachable_from(self, entries: &[&str]) -> Self;
    // Change-producing terminals
    pub fn delete(self) -> Changes;
    pub fn rename(self, f: impl Fn(&FuncDecl) -> String) -> Changes;
    pub fn replace_body(self, f: impl Fn(&FuncDecl) -> Block) -> Changes;
}

// MethodDecl
impl<'repo> Selection<'repo, MethodDecl> {
    pub fn named(self, name: &str) -> Self;
    pub fn on_type(self, type_name: &str) -> Self;
    pub fn calling(self, fqn: &str) -> Self;
    pub fn unreachable_from(self, entries: &[&str]) -> Self;
    // Change-producing terminals
    pub fn delete(self) -> Changes;
    pub fn rename(self, f: impl Fn(&MethodDecl) -> String) -> Changes;
    pub fn replace_body(self, f: impl Fn(&MethodDecl) -> Block) -> Changes;
}

// TypeSpec
impl<'repo> Selection<'repo, TypeSpec> {
    pub fn named(self, name: &str) -> Self;
    pub fn structs(self) -> Self;
    pub fn interfaces(self) -> Self;
    pub fn implementing(self, iface_fqn: &str) -> Self;
    // Entry API — returns MethodEntry selection
    pub fn method(self, name: &str) -> Selection<'repo, MethodEntry<'repo>>;
    // Change-producing terminals
    pub fn delete(self) -> Changes;
    pub fn rename(self, f: impl Fn(&TypeSpec) -> String) -> Changes;
    pub fn add_field(self, f: impl Fn(&TypeSpec) -> FieldDecl) -> Changes;
    pub fn remove_field(self, field_name: &str) -> Changes;
}
```

### `MethodEntry` and its selection

```rust
/// One entry per type in the parent selection.
/// Analogous to std::collections::hash_map::Entry.
pub struct MethodEntry<'repo> {
    pub ty: &'repo TypeSpec,
    pub existing: Option<&'repo MethodDecl>,
}

impl<'repo> Selection<'repo, MethodEntry<'repo>> {
    /// Add method where absent. No-op where already present.
    pub fn or_add(self, gen: impl Fn(&TypeSpec) -> MethodDecl) -> Changes;

    /// Modify method where present. No-op where absent.
    pub fn and_modify(self, gen: impl Fn(&MethodDecl) -> MethodDecl) -> Changes;

    /// Delete method where present. No-op where absent.
    pub fn delete(self) -> Changes;

    /// Read-only: collect only the entries where the method exists.
    pub fn existing(self) -> Selection<'repo, MethodDecl>;

    /// Read-only: collect only the types where the method is absent.
    pub fn absent(self) -> Selection<'repo, TypeSpec>;
}
```

### `Changes`

Pure data. No side effects. Can be combined before applying.

```rust
pub struct Changes {
    edits: Vec<Edit>,   // internal
}

impl Changes {
    pub fn none() -> Self { Self { edits: vec![] } }

    /// Combine an iterator of Changes into one.
    pub fn combine(iter: impl IntoIterator<Item = Changes>) -> Self;

    /// Combine two Changes. Sugar for Changes::combine([self, other]).
    pub fn and(self, other: Changes) -> Self;

    pub fn is_empty(&self) -> bool;
    pub fn edit_count(&self) -> usize;
}
```

### `Applied`

```rust
pub struct Applied<'repo> {
    repo: &'repo Repo,
    // per-file new source bytes, computed from edits
    results: HashMap<PathBuf, Vec<u8>>,
}

impl<'repo> Applied<'repo> {
    /// Print a unified diff to stdout. Returns self for chaining.
    pub fn preview(self) -> Self;

    /// Return modified source as strings without writing to disk.
    pub fn dry_run(&self) -> HashMap<PathBuf, String>;

    pub fn affected_files(&self) -> Vec<&Path>;
    pub fn edit_count(&self) -> usize;

    /// Write all modified files to disk.
    pub fn commit(self) -> Result<CommitSummary>;
}

pub struct CommitSummary {
    pub files_modified: usize,
    pub edits_applied: usize,
}
```

### Printer (internal)

Renders go-model types to source strings. Output is piped through `gofmt`.

```rust
// internal to go-analyzer, not pub
pub(crate) struct Printer;

impl Printer {
    pub fn method_decl(m: &MethodDecl) -> String;
    pub fn func_decl(f: &FuncDecl) -> String;
    pub fn type_spec(t: &TypeSpec) -> String;
    pub fn stmt(s: &Stmt) -> String;
    pub fn expr(e: &Expr) -> String;
    pub fn type_expr(t: &TypeExpr) -> String;
    pub fn gofmt(src: &str) -> String;
}
```

Precedence table for `Printer::expr` — the Go spec defines 5 levels,
highest to lowest:

```
5  *  /  %  <<  >>  &  &^
4  +  -  |  ^
3  ==  !=  <  <=  >  >=
2  &&
1  ||
```

`needs_parens(parent: BinaryOp, child: &Expr) -> bool` returns true when a
`Binary` child has lower precedence than its parent. This is the only place
parentheses are inserted — everywhere else the printer emits without parens.

---

## Complete usage examples

### Delete all `String()` methods on structs

```rust
let repo = Repo::load(".")?;

let changes = repo.structs()
    .method("String")
    .delete();

repo.apply(changes).commit()?;
```

### Add `String()` where missing

```rust
let repo = Repo::load(".")?;

let changes = repo.structs()
    .exported()
    .method("String")
    .or_add(|t| {
        let name = t.name().name.as_str();
        build::method(
            build::pointer_receiver("x", name),
            "String",
            vec![],
            vec![build::unnamed_param(build::named("string"))],
            build::block(vec![
                build::ret(vec![
                    build::call(
                        build::selector(build::ident("fmt"), "Sprintf"),
                        vec![
                            build::string("%+v"),
                            build::deref(build::ident("x")),
                        ],
                    )
                ])
            ]),
        )
    });

repo.apply(changes).preview().commit()?;
```

### Combine multiple changes in one commit

```rust
let repo = Repo::load(".")?;

let c1 = repo.structs()
    .method("String")
    .delete();

let c2 = repo.functions()
    .named("OldName")
    .rename(|_| "NewName".into());

let c3 = repo.types()
    .named("LegacyClient")
    .delete();

repo.apply(Changes::combine([c1, c2, c3]))
    .preview()
    .commit()?;
```

### Find dead code and remove it

```rust
let repo = Repo::load(".")?;

let c1 = repo.functions()
    .unreachable_from(&["main.main"])
    .excluding_tests()
    .delete();

let c2 = repo.methods()
    .unreachable_from(&["main.main"])
    .excluding_tests()
    .delete();

let applied = repo.apply(c1.and(c2));
println!("Removing {} edits across {} files",
    applied.edit_count(), applied.affected_files().len());
applied.commit()?;
```

### Read-only query — no changes

```rust
let repo = Repo::load(".")?;

// Count methods per struct
for t in repo.structs().collect() {
    let method_count = repo.methods()
        .on_type(t.name().name.as_str())
        .count();
    println!("{}: {} methods", t.name().name, method_count);
}

// All exported functions with no parameters
let fns = repo.functions()
    .exported()
    .filter(|f| f.ty.params.is_empty())
    .collect();
```

---

## Walker: tree-sitter node kind → go-model

Statement mapping (tree-sitter kind → `Stmt` variant):

```
block                       → Stmt::Block
assignment_statement        → Stmt::Assign
short_var_declaration       → Stmt::ShortVarDecl
inc_statement               → Stmt::Inc
dec_statement               → Stmt::Dec
send_statement              → Stmt::Send
return_statement            → Stmt::Return
if_statement                → Stmt::If
for_statement               → Stmt::For or Stmt::ForRange
expression_switch_statement → Stmt::Switch
type_switch_statement       → Stmt::TypeSwitch
select_statement            → Stmt::Select
go_statement                → Stmt::Go
defer_statement             → Stmt::Defer
labeled_statement           → Stmt::Labeled
fallthrough_statement       → Stmt::Fallthrough
empty_statement             → Stmt::Empty
break_statement             → Stmt::Break
continue_statement          → Stmt::Continue
goto_statement              → Stmt::Goto
```

---

## Implementation order

1. **`go-model`** — all types, `build::*`, `Span::synthetic()`.
   Test: construct a `MethodDecl` with `build::method`, assert all fields.

2. **Printer** (inside `go-analyzer`) — bottom-up: `type_expr`, `expr`,
   `stmt`, `method_decl`. Test: print each node kind, parse result via
   `go/parser` subprocess to verify syntactic validity. Test `needs_parens`
   for all precedence combinations.

3. **Walker** — extend to fill full `Block`/`Stmt`/`Expr`.
   Roundtrip test: `parse → model → print → parse`, assert no errors.
   Run against `golang/go` stdlib (5,670 files).

4. **Resolver** — import alias resolution, qualified call resolution.

5. **Fluent API** — `Repo`, `Selection<T>`, `MethodEntry`, `Changes`, `Applied`.
   Test: load a fixture repo, run each selection filter, assert counts.

6. **Edit engine** — `Edit`, `apply`, `commit`, blank-line cleanup on deletion.
   Test: delete a method, verify output compiles with `go build`.
   Test: `or_add`, verify inserted method compiles and `gofmt` accepts it.

7. **CLI** — thin binary, one subcommand per query type, `--dry-run` flag.
