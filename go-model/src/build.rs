use crate::{
    Block, Expr, FuncType, Ident, IntLit, MethodDecl, ParamDecl, Receiver, Span, Stmt, StringLit,
    TypeExpr, UnaryOp,
};

/// Creates a named type expression from a string.
///
/// ```
/// # use go_model::*;
/// let ty = build::named("int");
/// assert!(matches!(ty, TypeExpr::Named(_)));
/// ```
pub fn named(name: &str) -> TypeExpr {
    TypeExpr::Named(Ident::synthetic(name))
}

/// Creates a pointer type expression (`*T`).
///
/// ```
/// # use go_model::*;
/// let ty = build::pointer(build::named("int"));
/// assert!(matches!(ty, TypeExpr::Pointer(_)));
/// ```
pub fn pointer(inner: TypeExpr) -> TypeExpr {
    TypeExpr::Pointer(Box::new(inner))
}

/// Creates a slice type expression (`[]T`).
///
/// ```
/// # use go_model::*;
/// let ty = build::slice(build::named("byte"));
/// assert!(matches!(ty, TypeExpr::Slice(_)));
/// ```
pub fn slice(elem: TypeExpr) -> TypeExpr {
    TypeExpr::Slice(Box::new(elem))
}

/// Creates a map type expression (`map[K]V`).
///
/// ```
/// # use go_model::*;
/// let ty = build::map_type(build::named("string"), build::named("int"));
/// assert!(matches!(ty, TypeExpr::Map { .. }));
/// ```
pub fn map_type(key: TypeExpr, value: TypeExpr) -> TypeExpr {
    TypeExpr::Map {
        key: Box::new(key),
        value: Box::new(value),
    }
}

/// Creates an identifier expression.
///
/// ```
/// # use go_model::*;
/// let e = build::ident("x");
/// assert!(matches!(e, Expr::Ident(_)));
/// ```
pub fn ident(name: &str) -> Expr {
    Expr::Ident(Ident::synthetic(name))
}

/// Creates a string literal expression from an unescaped value.
///
/// ```
/// # use go_model::*;
/// let e = build::string("hello");
/// assert!(matches!(e, Expr::String(_)));
/// ```
pub fn string(value: &str) -> Expr {
    Expr::String(StringLit::from_value(value))
}

/// Creates an integer literal expression.
///
/// ```
/// # use go_model::*;
/// let e = build::int(42);
/// assert!(matches!(e, Expr::Int(_)));
/// ```
pub fn int(value: i64) -> Expr {
    Expr::Int(IntLit {
        raw: value.to_string(),
        span: Span::synthetic(),
    })
}

/// Creates a function call expression.
///
/// ```
/// # use go_model::*;
/// let e = build::call(build::ident("println"), vec![build::string("hi")]);
/// assert!(matches!(e, Expr::Call { .. }));
/// ```
pub fn call(func: Expr, args: Vec<Expr>) -> Expr {
    Expr::Call {
        func: Box::new(func),
        type_args: vec![],
        args,
        ellipsis: false,
        span: Span::synthetic(),
    }
}

/// Creates a selector expression (`operand.field`).
///
/// ```
/// # use go_model::*;
/// let e = build::selector(build::ident("fmt"), "Println");
/// assert!(matches!(e, Expr::Selector { .. }));
/// ```
pub fn selector(operand: Expr, field: &str) -> Expr {
    Expr::Selector {
        operand: Box::new(operand),
        field: Ident::synthetic(field),
        span: Span::synthetic(),
    }
}

/// Creates a pointer dereference expression (`*operand`).
///
/// ```
/// # use go_model::*;
/// let e = build::deref(build::ident("p"));
/// assert!(matches!(e, Expr::Unary { op: UnaryOp::Deref, .. }));
/// ```
pub fn deref(operand: Expr) -> Expr {
    Expr::Unary {
        op: UnaryOp::Deref,
        operand: Box::new(operand),
        span: Span::synthetic(),
    }
}

/// Creates an address-of expression (`&operand`).
///
/// ```
/// # use go_model::*;
/// let e = build::addr(build::ident("x"));
/// assert!(matches!(e, Expr::Unary { op: UnaryOp::Addr, .. }));
/// ```
pub fn addr(operand: Expr) -> Expr {
    Expr::Unary {
        op: UnaryOp::Addr,
        operand: Box::new(operand),
        span: Span::synthetic(),
    }
}

/// Creates a return statement.
///
/// ```
/// # use go_model::*;
/// let s = build::ret(vec![build::ident("nil")]);
/// assert!(matches!(s, Stmt::Return { .. }));
/// ```
pub fn ret(values: Vec<Expr>) -> Stmt {
    Stmt::Return {
        values,
        span: Span::synthetic(),
    }
}

/// Creates a block of statements.
///
/// ```
/// # use go_model::*;
/// let b = build::block(vec![build::ret(vec![])]);
/// assert_eq!(b.stmts.len(), 1);
/// ```
pub fn block(stmts: Vec<Stmt>) -> Block {
    Block {
        stmts,
        span: Span::synthetic(),
    }
}

/// Creates a named parameter declaration.
///
/// ```
/// # use go_model::*;
/// let p = build::param(&["x", "y"], build::named("int"));
/// assert_eq!(p.names.len(), 2);
/// ```
pub fn param(names: &[&str], ty: TypeExpr) -> ParamDecl {
    ParamDecl {
        names: names.iter().map(|n| Ident::synthetic(n)).collect(),
        ty,
        variadic: false,
        span: Span::synthetic(),
    }
}

/// Creates an unnamed parameter declaration (type only, no parameter name).
///
/// ```
/// # use go_model::*;
/// let p = build::unnamed_param(build::named("error"));
/// assert!(p.names.is_empty());
/// ```
pub fn unnamed_param(ty: TypeExpr) -> ParamDecl {
    ParamDecl {
        names: vec![],
        ty,
        variadic: false,
        span: Span::synthetic(),
    }
}

/// Creates a pointer receiver, e.g. `(s *Server)`.
///
/// ```
/// # use go_model::*;
/// let r = build::pointer_receiver("s", "Server");
/// assert!(matches!(r.ty, TypeExpr::Pointer(_)));
/// ```
pub fn pointer_receiver(var_name: &str, type_name: &str) -> Receiver {
    Receiver {
        name: Some(Ident::synthetic(var_name)),
        type_params: vec![],
        ty: pointer(named(type_name)),
        span: Span::synthetic(),
    }
}

/// Creates a value receiver, e.g. `(s Server)`.
///
/// ```
/// # use go_model::*;
/// let r = build::value_receiver("s", "Server");
/// assert!(matches!(r.ty, TypeExpr::Named(_)));
/// ```
pub fn value_receiver(var_name: &str, type_name: &str) -> Receiver {
    Receiver {
        name: Some(Ident::synthetic(var_name)),
        type_params: vec![],
        ty: named(type_name),
        span: Span::synthetic(),
    }
}

/// Creates a method declaration with a receiver, parameters, results, and body.
///
/// ```
/// # use go_model::*;
/// let m = build::method(
///     build::pointer_receiver("s", "Server"),
///     "Start",
///     vec![],
///     vec![build::unnamed_param(build::named("error"))],
///     build::block(vec![build::ret(vec![build::ident("nil")])]),
/// );
/// assert_eq!(m.name.name, "Start");
/// ```
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
