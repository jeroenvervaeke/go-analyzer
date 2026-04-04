use serde::{Deserialize, Serialize};

use crate::{ConstSpec, Expr, Ident, Span, TypeExpr, TypeSpec, VarSpec};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    Block(Block),
    Expr(Expr, Span),
    Assign {
        lhs: Vec<Expr>,
        op: AssignOp,
        rhs: Vec<Expr>,
        span: Span,
    },
    ShortVarDecl {
        names: Vec<Ident>,
        values: Vec<Expr>,
        span: Span,
    },
    VarDecl(VarSpec, Span),
    ConstDecl(ConstSpec, Span),
    Inc(Expr, Span),
    Dec(Expr, Span),
    Send {
        channel: Expr,
        value: Expr,
        span: Span,
    },
    Return {
        values: Vec<Expr>,
        span: Span,
    },
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
        iterable: Box<Expr>,
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
    Select {
        cases: Vec<CommCase>,
        span: Span,
    },
    Go(Expr, Span),
    Defer(Expr, Span),
    Break(Option<Ident>, Span),
    Continue(Option<Ident>, Span),
    Goto(Ident, Span),
    Fallthrough(Span),
    Labeled {
        label: Ident,
        body: Box<Stmt>,
        span: Span,
    },
    TypeDecl(TypeSpec, Span),
    Empty(Span),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    RemAssign,
    AndAssign,
    OrAssign,
    XorAssign,
    AndNotAssign,
    ShlAssign,
    ShrAssign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RangeAssign {
    Define,
    Assign,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExprCase {
    pub exprs: Vec<Expr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeCase {
    pub types: Vec<TypeExpr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeSwitchAssign {
    pub name: Option<Ident>,
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommCase {
    Send {
        stmt: Stmt,
        body: Vec<Stmt>,
        span: Span,
    },
    Recv {
        stmt: Option<Stmt>,
        body: Vec<Stmt>,
        span: Span,
    },
    Default {
        body: Vec<Stmt>,
        span: Span,
    },
}
