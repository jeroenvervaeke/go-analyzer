use serde::{Deserialize, Serialize};

use crate::{Expr, FuncType, Ident, InterfaceType, StructType};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeExpr {
    Named(Ident),
    Qualified {
        package: Ident,
        name: Ident,
    },
    Pointer(Box<TypeExpr>),
    Array {
        len: Box<Expr>,
        elem: Box<TypeExpr>,
    },
    Slice(Box<TypeExpr>),
    Map {
        key: Box<TypeExpr>,
        value: Box<TypeExpr>,
    },
    Channel {
        direction: ChanDir,
        elem: Box<TypeExpr>,
    },
    Func(FuncType),
    Interface(InterfaceType),
    Struct(StructType),
    Generic {
        base: Box<TypeExpr>,
        args: Vec<TypeExpr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChanDir {
    Both,
    Recv,
    Send,
}
