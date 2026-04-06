use serde::{Deserialize, Serialize};

use crate::{
    Block, FloatLit, FuncType, Ident, ImaginaryLit, IntLit, RawStringLit, RuneLit, Span, StringLit,
    TypeExpr,
};

/// Represents every Go expression (identifiers, literals, calls, selectors, operators, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// Simple identifier, e.g. `x`, `fmt`.
    Ident(Ident),
    /// Qualified identifier, e.g. `fmt.Println`.
    Qualified {
        package: Ident,
        name: Ident,
        span: Span,
    },
    /// Integer literal.
    Int(IntLit),
    /// Floating-point literal.
    Float(FloatLit),
    /// Imaginary literal, e.g. `2i`.
    Imaginary(ImaginaryLit),
    /// Rune literal, e.g. `'a'`.
    Rune(RuneLit),
    /// Interpreted string literal, e.g. `"hello\n"`.
    String(StringLit),
    /// Raw string literal, e.g. `` `hello` ``.
    RawString(RawStringLit),
    /// Boolean `true`.
    True(Span),
    /// Boolean `false`.
    False(Span),
    /// The `nil` value.
    Nil(Span),
    /// The `iota` constant generator.
    Iota(Span),
    /// Composite literal, e.g. `Point{X: 1, Y: 2}`.
    Composite {
        ty: Box<TypeExpr>,
        elems: Vec<KeyedElem>,
        span: Span,
    },
    /// Function literal (closure), e.g. `func(x int) { ... }`.
    FuncLit {
        ty: FuncType,
        body: Block,
        span: Span,
    },
    /// Function or method call, e.g. `f(x)`, `obj.Method(a, b...)`.
    Call {
        func: Box<Expr>,
        type_args: Vec<TypeExpr>,
        args: Vec<Expr>,
        /// Whether the last argument is variadic (`...`).
        ellipsis: bool,
        span: Span,
    },
    /// Field or method selector, e.g. `obj.Field`.
    Selector {
        operand: Box<Expr>,
        field: Ident,
        span: Span,
    },
    /// Index expression, e.g. `a[i]`.
    Index {
        operand: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    /// Slice expression, e.g. `a[low:high]` or `a[low:high:max]`.
    Slice {
        operand: Box<Expr>,
        low: Option<Box<Expr>>,
        high: Option<Box<Expr>>,
        max: Option<Box<Expr>>,
        span: Span,
    },
    /// Type assertion, e.g. `x.(T)`.
    TypeAssert {
        operand: Box<Expr>,
        ty: Box<TypeExpr>,
        span: Span,
    },
    /// Unary operation, e.g. `!x`, `*p`, `&v`.
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    /// Binary operation, e.g. `a + b`, `x && y`.
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    /// Parenthesized expression, e.g. `(x + y)`.
    Paren(Box<Expr>, Span),
}

/// Represents an element in a composite literal, optionally keyed (e.g., `Key: Value`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyedElem {
    pub key: Option<Expr>,
    pub value: Expr,
    pub span: Span,
}

/// Represents a Go unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    /// Logical NOT (`!`).
    Not,
    /// Arithmetic negation (`-`).
    Neg,
    /// Unary plus (`+`).
    Pos,
    /// Pointer dereference (`*`).
    Deref,
    /// Address-of (`&`).
    Addr,
    /// Channel receive (`<-`).
    Recv,
    /// Bitwise NOT (`^`).
    BitNot,
}

/// Represents a Go binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    /// Addition (`+`).
    Add,
    /// Subtraction (`-`).
    Sub,
    /// Multiplication (`*`).
    Mul,
    /// Division (`/`).
    Div,
    /// Remainder (`%`).
    Rem,
    /// Bitwise AND (`&`).
    And,
    /// Bitwise OR (`|`).
    Or,
    /// Bitwise XOR (`^`).
    Xor,
    /// Bit clear / AND NOT (`&^`).
    AndNot,
    /// Left shift (`<<`).
    Shl,
    /// Right shift (`>>`).
    Shr,
    /// Logical AND (`&&`).
    LogAnd,
    /// Logical OR (`||`).
    LogOr,
    /// Equal (`==`).
    Eq,
    /// Not equal (`!=`).
    Ne,
    /// Less than (`<`).
    Lt,
    /// Less than or equal (`<=`).
    Le,
    /// Greater than (`>`).
    Gt,
    /// Greater than or equal (`>=`).
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
