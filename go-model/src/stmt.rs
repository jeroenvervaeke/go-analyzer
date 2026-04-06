use serde::{Deserialize, Serialize};

use crate::{ConstSpec, Expr, Ident, Span, TypeExpr, TypeSpec, VarSpec};

/// Represents every Go statement (if, for, switch, select, return, assign, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    /// Bare block statement `{ ... }`.
    Block(Block),
    /// Expression statement (e.g., a standalone function call).
    Expr(Expr, Span),
    /// Assignment statement, e.g. `x = 1`, `a, b += 2, 3`.
    Assign {
        lhs: Vec<Expr>,
        op: AssignOp,
        rhs: Vec<Expr>,
        span: Span,
    },
    /// Short variable declaration, e.g. `x := 1`.
    ShortVarDecl {
        names: Vec<Ident>,
        values: Vec<Expr>,
        span: Span,
    },
    /// `var` declaration inside a function body.
    VarDecl(Vec<VarSpec>, Span),
    /// `const` declaration inside a function body.
    ConstDecl(Vec<ConstSpec>, Span),
    /// Increment statement, e.g. `x++`.
    Inc(Expr, Span),
    /// Decrement statement, e.g. `x--`.
    Dec(Expr, Span),
    /// Channel send statement, e.g. `ch <- v`.
    Send {
        channel: Expr,
        value: Expr,
        span: Span,
    },
    /// Return statement, e.g. `return x, nil`.
    Return { values: Vec<Expr>, span: Span },
    /// `if` statement with optional init and else clause.
    If {
        init: Option<Box<Stmt>>,
        cond: Expr,
        body: Block,
        else_: Option<Box<Stmt>>,
        span: Span,
    },
    /// C-style `for` loop, e.g. `for i := 0; i < n; i++ { ... }`.
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        post: Option<Box<Stmt>>,
        body: Block,
        span: Span,
    },
    /// `for ... range` loop, e.g. `for k, v := range m { ... }`.
    ForRange {
        key: Option<Expr>,
        value: Option<Expr>,
        assign: RangeAssign,
        iterable: Box<Expr>,
        body: Block,
        span: Span,
    },
    /// Expression `switch` statement.
    Switch {
        init: Option<Box<Stmt>>,
        tag: Option<Expr>,
        cases: Vec<ExprCase>,
        span: Span,
    },
    /// Type `switch` statement, e.g. `switch v := x.(type) { ... }`.
    TypeSwitch {
        init: Option<Box<Stmt>>,
        assign: TypeSwitchAssign,
        cases: Vec<TypeCase>,
        span: Span,
    },
    /// `select` statement for channel operations.
    Select { cases: Vec<CommCase>, span: Span },
    /// `go` statement (goroutine launch).
    Go(Expr, Span),
    /// `defer` statement.
    Defer(Expr, Span),
    /// `break` statement with optional label.
    Break(Option<Ident>, Span),
    /// `continue` statement with optional label.
    Continue(Option<Ident>, Span),
    /// `goto` statement.
    Goto(Ident, Span),
    /// `fallthrough` statement inside a switch case.
    Fallthrough(Span),
    /// Labeled statement, e.g. `Loop: for { ... }`.
    Labeled {
        label: Ident,
        body: Box<Stmt>,
        span: Span,
    },
    /// Type declaration inside a function body.
    TypeDecl(Vec<TypeSpec>, Span),
    /// Empty statement (`;`).
    Empty(Span),
}

/// Represents a Go assignment operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignOp {
    /// Simple assignment (`=`).
    Assign,
    /// Add-assign (`+=`).
    AddAssign,
    /// Subtract-assign (`-=`).
    SubAssign,
    /// Multiply-assign (`*=`).
    MulAssign,
    /// Divide-assign (`/=`).
    DivAssign,
    /// Remainder-assign (`%=`).
    RemAssign,
    /// Bitwise AND-assign (`&=`).
    AndAssign,
    /// Bitwise OR-assign (`|=`).
    OrAssign,
    /// Bitwise XOR-assign (`^=`).
    XorAssign,
    /// Bit-clear assign (`&^=`).
    AndNotAssign,
    /// Left-shift assign (`<<=`).
    ShlAssign,
    /// Right-shift assign (`>>=`).
    ShrAssign,
}

/// Represents the assignment mode in a `for ... range` loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RangeAssign {
    /// Short variable declaration (`:=`).
    Define,
    /// Plain assignment (`=`).
    Assign,
    /// Bare `for range x` with no iteration variables (Go 1.22+).
    None,
}

/// Represents a brace-delimited block of statements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

/// Represents a `case` clause in an expression `switch` statement.
///
/// An empty `exprs` list denotes the `default` case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExprCase {
    pub exprs: Vec<Expr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Represents a `case` clause in a type `switch` statement.
///
/// An empty `types` list denotes the `default` case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeCase {
    pub types: Vec<TypeExpr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Represents the assignment in a type switch guard, e.g. `v := x.(type)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeSwitchAssign {
    /// The variable name if `:=` is used, `None` for a bare `x.(type)`.
    pub name: Option<Ident>,
    pub expr: Expr,
    pub span: Span,
}

/// Represents a `case` clause in a `select` statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommCase {
    /// Send case, e.g. `case ch <- v:`.
    Send {
        stmt: Box<Stmt>,
        body: Vec<Stmt>,
        span: Span,
    },
    /// Receive case, e.g. `case x := <-ch:` or bare `case <-ch:`.
    Recv {
        /// Assignment statement if `x := <-ch` or `x = <-ch`, otherwise None.
        stmt: Option<Box<Stmt>>,
        /// The receive expression (e.g., `<-ch`) for bare receives without assignment.
        recv_expr: Option<Box<Expr>>,
        body: Vec<Stmt>,
        span: Span,
    },
    /// Default case.
    Default { body: Vec<Stmt>, span: Span },
}
