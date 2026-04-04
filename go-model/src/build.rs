use crate::{
    Block, Expr, FuncType, Ident, IntLit, MethodDecl, ParamDecl, Receiver, Span, Stmt, StringLit,
    TypeExpr, UnaryOp,
};

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
    TypeExpr::Map {
        key: Box::new(key),
        value: Box::new(value),
    }
}

// --- exprs ---

pub fn ident(name: &str) -> Expr {
    Expr::Ident(Ident::synthetic(name))
}

pub fn string(value: &str) -> Expr {
    Expr::String(StringLit::from_value(value))
}

pub fn int(value: i64) -> Expr {
    Expr::Int(IntLit {
        raw: value.to_string(),
        span: Span::synthetic(),
    })
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
    Expr::Unary {
        op: UnaryOp::Deref,
        operand: Box::new(operand),
        span: Span::synthetic(),
    }
}

pub fn addr(operand: Expr) -> Expr {
    Expr::Unary {
        op: UnaryOp::Addr,
        operand: Box::new(operand),
        span: Span::synthetic(),
    }
}

// --- stmts ---

pub fn ret(values: Vec<Expr>) -> Stmt {
    Stmt::Return {
        values,
        span: Span::synthetic(),
    }
}

pub fn block(stmts: Vec<Stmt>) -> Block {
    Block {
        stmts,
        span: Span::synthetic(),
    }
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
    ParamDecl {
        names: vec![],
        ty,
        variadic: false,
        span: Span::synthetic(),
    }
}

pub fn pointer_receiver(var_name: &str, type_name: &str) -> Receiver {
    Receiver {
        name: Some(Ident::synthetic(var_name)),
        type_params: vec![],
        ty: pointer(named(type_name)),
        span: Span::synthetic(),
    }
}

pub fn value_receiver(var_name: &str, type_name: &str) -> Receiver {
    Receiver {
        name: Some(Ident::synthetic(var_name)),
        type_params: vec![],
        ty: named(type_name),
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
