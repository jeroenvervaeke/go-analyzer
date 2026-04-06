use serde::{Deserialize, Serialize};

use crate::{
    Block, FloatLit, FuncType, Ident, ImaginaryLit, IntLit, RawStringLit, RuneLit, Span, StringLit,
    TypeExpr,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Ident(Ident),
    Qualified {
        package: Ident,
        name: Ident,
        span: Span,
    },
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
    Composite {
        ty: Box<TypeExpr>,
        elems: Vec<KeyedElem>,
        span: Span,
    },
    FuncLit {
        ty: FuncType,
        body: Block,
        span: Span,
    },
    Call {
        func: Box<Expr>,
        type_args: Vec<TypeExpr>,
        args: Vec<Expr>,
        ellipsis: bool,
        span: Span,
    },
    Selector {
        operand: Box<Expr>,
        field: Ident,
        span: Span,
    },
    Index {
        operand: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Slice {
        operand: Box<Expr>,
        low: Option<Box<Expr>>,
        high: Option<Box<Expr>>,
        max: Option<Box<Expr>>,
        span: Span,
    },
    TypeAssert {
        operand: Box<Expr>,
        ty: Box<TypeExpr>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Paren(Box<Expr>, Span),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyedElem {
    pub key: Option<Expr>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,
    Neg,
    Pos,
    Deref,
    Addr,
    Recv,
    BitNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
    AndNot,
    Shl,
    Shr,
    LogAnd,
    LogOr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinaryOp {
    /// Go spec precedence level (1 = lowest, 5 = highest).
    pub fn precedence(self) -> u8 {
        match self {
            Self::LogOr => 1,
            Self::LogAnd => 2,
            Self::Eq | Self::Ne | Self::Lt | Self::Le | Self::Gt | Self::Ge => 3,
            Self::Add | Self::Sub | Self::Or | Self::Xor => 4,
            Self::Mul
            | Self::Div
            | Self::Rem
            | Self::And
            | Self::AndNot
            | Self::Shl
            | Self::Shr => 5,
        }
    }
}
