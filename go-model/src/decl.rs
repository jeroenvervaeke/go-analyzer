use serde::{Deserialize, Serialize};

use crate::{Block, Expr, Ident, Span, StringLit, TypeExpr};

/// Represents a Go function signature (parameters, results, and optional type parameters).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuncType {
    pub type_params: Vec<TypeParam>,
    pub params: Vec<ParamDecl>,
    pub results: Vec<ParamDecl>,
    pub span: Span,
}

impl FuncType {
    /// Returns true if two function signatures are compatible, ignoring
    /// parameter names and spans. Compares only the types, variadic flags,
    /// and return types — the things that matter for interface satisfaction.
    pub fn signature_matches(&self, other: &FuncType) -> bool {
        params_match(&self.params, &other.params) && params_match(&self.results, &other.results)
    }
}

/// Compare two parameter lists structurally, ignoring names and spans.
fn params_match(a: &[ParamDecl], b: &[ParamDecl]) -> bool {
    let a_flat = flatten_params(a);
    let b_flat = flatten_params(b);

    if a_flat.len() != b_flat.len() {
        return false;
    }

    a_flat
        .iter()
        .zip(b_flat.iter())
        .all(|((ty_a, var_a), (ty_b, var_b))| type_eq(ty_a, ty_b) && var_a == var_b)
}

/// Flatten parameter lists: `(a, b int, c string)` → `[(int, false), (int, false), (string, false)]`.
/// Unnamed params like `(int, error)` stay as-is.
fn flatten_params(params: &[ParamDecl]) -> Vec<(&TypeExpr, bool)> {
    let mut result = Vec::new();
    for p in params {
        let count = if p.names.is_empty() { 1 } else { p.names.len() };
        for _ in 0..count {
            result.push((&p.ty, p.variadic));
        }
    }
    result
}

/// Compare two type expressions structurally, ignoring all spans.
/// This is needed because the derived `PartialEq` on `TypeExpr` includes
/// `Ident.span`, making two identical types from different source positions
/// compare as unequal.
pub fn type_eq(a: &TypeExpr, b: &TypeExpr) -> bool {
    match (a, b) {
        (TypeExpr::Named(a), TypeExpr::Named(b)) => a.name == b.name,
        (
            TypeExpr::Qualified {
                package: pa,
                name: na,
            },
            TypeExpr::Qualified {
                package: pb,
                name: nb,
            },
        ) => pa.name == pb.name && na.name == nb.name,
        (TypeExpr::Pointer(a), TypeExpr::Pointer(b)) => type_eq(a, b),
        (TypeExpr::Array { len: _, elem: ea }, TypeExpr::Array { len: _, elem: eb }) => {
            // Array lengths are expressions — for signature matching we compare
            // the element types only. This is a simplification; exact length
            // matching would require expression equality.
            type_eq(ea, eb)
        }
        (TypeExpr::Slice(a), TypeExpr::Slice(b)) => type_eq(a, b),
        (TypeExpr::Map { key: ka, value: va }, TypeExpr::Map { key: kb, value: vb }) => {
            type_eq(ka, kb) && type_eq(va, vb)
        }
        (
            TypeExpr::Channel {
                direction: da,
                elem: ea,
            },
            TypeExpr::Channel {
                direction: db,
                elem: eb,
            },
        ) => da == db && type_eq(ea, eb),
        (TypeExpr::Func(a), TypeExpr::Func(b)) => {
            params_match(&a.params, &b.params) && params_match(&a.results, &b.results)
        }
        (TypeExpr::Interface(a), TypeExpr::Interface(b)) => {
            // Simplified: compare element count and names only
            a.elements.len() == b.elements.len()
        }
        (TypeExpr::Struct(a), TypeExpr::Struct(b)) => a.fields.len() == b.fields.len(),
        (TypeExpr::Generic { base: ba, args: aa }, TypeExpr::Generic { base: bb, args: ab }) => {
            type_eq(ba, bb)
                && aa.len() == ab.len()
                && aa.iter().zip(ab.iter()).all(|(a, b)| type_eq(a, b))
        }
        _ => false,
    }
}

/// Represents a parameter declaration in a function signature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamDecl {
    pub names: Vec<Ident>,
    pub ty: TypeExpr,
    pub variadic: bool,
    pub span: Span,
}

/// Represents a type parameter declaration in a generic function or type, e.g. `[T any]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeParam {
    pub names: Vec<Ident>,
    pub constraint: TypeExpr,
    pub span: Span,
}

/// Represents a method receiver, e.g. `(s *Server)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receiver {
    pub name: Option<Ident>,
    pub type_params: Vec<TypeParam>,
    pub ty: TypeExpr,
    pub span: Span,
}

/// Represents a top-level function declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuncDecl {
    pub name: Ident,
    pub ty: FuncType,
    pub body: Option<Block>,
    pub doc: Option<String>,
    pub span: Span,
}

/// Represents a method declaration (a function with a receiver).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodDecl {
    pub receiver: Receiver,
    pub name: Ident,
    pub ty: FuncType,
    pub body: Option<Block>,
    pub doc: Option<String>,
    pub span: Span,
}

/// Represents a Go struct type with its field declarations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructType {
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

/// Represents a field declaration within a struct type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldDecl {
    /// Named field(s), e.g. `X, Y int` or `Name string \`json:"name"\``.
    Named {
        names: Vec<Ident>,
        ty: TypeExpr,
        tag: Option<StringLit>,
        span: Span,
    },
    /// Embedded (anonymous) field, e.g. `io.Reader` or `*Base`.
    Embedded {
        ty: TypeExpr,
        tag: Option<StringLit>,
        span: Span,
    },
}

/// Represents a Go interface type with its method and type constraints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceType {
    pub elements: Vec<InterfaceElem>,
    pub span: Span,
}

/// Represents an element within an interface type definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InterfaceElem {
    /// Method signature, e.g. `Read([]byte) (int, error)`.
    Method {
        name: Ident,
        ty: FuncType,
        span: Span,
    },
    /// Type constraint term (Go 1.18+), e.g. `~int | ~string`.
    TypeTerm(TypeTerm),
    /// Embedded interface, e.g. `io.Reader`.
    Embedded(TypeExpr),
}

/// Represents a union of type constraint elements, e.g. `~int | ~string`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeTerm {
    pub terms: Vec<TypeTermElem>,
    pub span: Span,
}

/// Represents a single element in a type constraint union.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeTermElem {
    /// Whether the `~` (underlying type) prefix is present.
    pub tilde: bool,
    pub ty: TypeExpr,
    pub span: Span,
}

/// Represents a type specification (alias or definition).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeSpec {
    /// Type alias, e.g. `type Foo = Bar`.
    Alias {
        name: Ident,
        type_params: Vec<TypeParam>,
        ty: TypeExpr,
        span: Span,
    },
    /// Type definition, e.g. `type Foo struct { ... }`.
    Def {
        name: Ident,
        type_params: Vec<TypeParam>,
        ty: TypeExpr,
        span: Span,
    },
}

impl TypeSpec {
    /// Returns the declared type name.
    pub fn name(&self) -> &Ident {
        match self {
            Self::Alias { name, .. } | Self::Def { name, .. } => name,
        }
    }

    /// Returns the source span of this type spec.
    pub fn span(&self) -> Span {
        match self {
            Self::Alias { span, .. } | Self::Def { span, .. } => *span,
        }
    }

    /// Returns a reference to the underlying type expression.
    pub fn ty(&self) -> &TypeExpr {
        match self {
            Self::Alias { ty, .. } | Self::Def { ty, .. } => ty,
        }
    }

    /// Returns `true` if this is a struct type definition.
    pub fn is_struct(&self) -> bool {
        matches!(
            self,
            Self::Def {
                ty: TypeExpr::Struct(_),
                ..
            }
        )
    }

    /// Returns `true` if this is an interface type definition.
    pub fn is_interface(&self) -> bool {
        matches!(
            self,
            Self::Def {
                ty: TypeExpr::Interface(_),
                ..
            }
        )
    }
}

/// Represents a `var` specification, e.g. `var x, y int = 1, 2`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VarSpec {
    pub names: Vec<Ident>,
    pub ty: Option<TypeExpr>,
    pub values: Vec<Expr>,
    pub span: Span,
}

/// Represents a `const` specification, e.g. `const Pi float64 = 3.14`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstSpec {
    pub names: Vec<Ident>,
    pub ty: Option<TypeExpr>,
    pub values: Vec<Expr>,
    pub span: Span,
}

/// Represents a single import specification, e.g. `import "fmt"` or `import io "io/ioutil"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSpec {
    pub alias: ImportAlias,
    pub path: StringLit,
    pub span: Span,
}

/// Represents the alias form of an import declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImportAlias {
    /// No explicit alias; the package name is derived from the import path.
    Implicit,
    /// Dot import (`. "pkg"`), which imports all exported names into the current scope.
    Dot,
    /// Blank import (`_ "pkg"`), used solely for side effects.
    Blank,
    /// Explicit named alias, e.g. `mypkg "some/long/path"`.
    Named(Ident),
}

/// Represents a complete Go source file: package clause, imports, and top-level declarations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceFile {
    pub package: Ident,
    pub imports: Vec<ImportSpec>,
    pub decls: Vec<TopLevelDecl>,
    pub span: Span,
}

/// Represents a top-level declaration in a Go source file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TopLevelDecl {
    /// Top-level function declaration.
    Func(Box<FuncDecl>),
    /// Method declaration (function with receiver).
    Method(Box<MethodDecl>),
    /// Type declaration group (`type ( ... )`).
    Type(Vec<TypeSpec>),
    /// Variable declaration group (`var ( ... )`).
    Var(Vec<VarSpec>),
    /// Constant declaration group (`const ( ... )`).
    Const(Vec<ConstSpec>),
}
