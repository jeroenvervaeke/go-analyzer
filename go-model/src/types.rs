use serde::{Deserialize, Serialize};

use crate::{Expr, FuncType, Ident, InterfaceType, StructType};

/// Represents every Go type expression (named, pointer, slice, map, channel, func, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeExpr {
    /// Simple named type, e.g. `int`, `MyStruct`.
    Named(Ident),
    /// Qualified type from another package, e.g. `fmt.Stringer`.
    Qualified { package: Ident, name: Ident },
    /// Pointer type, e.g. `*T`.
    Pointer(Box<TypeExpr>),
    /// Fixed-length array type, e.g. `[4]byte`.
    Array { len: Box<Expr>, elem: Box<TypeExpr> },
    /// Slice type, e.g. `[]string`.
    Slice(Box<TypeExpr>),
    /// Map type, e.g. `map[string]int`.
    Map {
        key: Box<TypeExpr>,
        value: Box<TypeExpr>,
    },
    /// Channel type with direction, e.g. `chan int`, `<-chan int`.
    Channel {
        direction: ChanDir,
        elem: Box<TypeExpr>,
    },
    /// Function type, e.g. `func(int) error`.
    Func(FuncType),
    /// Interface type, e.g. `interface{ Read([]byte) (int, error) }`.
    Interface(InterfaceType),
    /// Struct type, e.g. `struct{ Name string }`.
    Struct(StructType),
    /// Generic instantiation, e.g. `Map[string, int]`.
    Generic {
        base: Box<TypeExpr>,
        args: Vec<TypeExpr>,
    },
}

/// Represents the direction of a Go channel type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChanDir {
    /// Bidirectional channel (`chan T`).
    Both,
    /// Receive-only channel (`<-chan T`).
    Recv,
    /// Send-only channel (`chan<- T`).
    Send,
}
