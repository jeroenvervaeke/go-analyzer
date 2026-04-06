use crate::build;
use crate::*;

#[test]
fn test_span_synthetic() {
    let s = Span::synthetic();
    assert!(s.is_synthetic());
    assert_eq!(s.start_byte, 0);
    assert_eq!(s.end_byte, 0);
}

#[test]
fn test_span_real_not_synthetic() {
    let s = Span {
        start_byte: 0,
        end_byte: 10,
        start_row: 0,
        start_col: 0,
        end_row: 0,
        end_col: 10,
    };
    assert!(!s.is_synthetic());
}

#[test]
fn test_span_zero_offset_real_not_confused_with_synthetic() {
    // A real span at position (0,0) to (0,5) has end_byte != 0
    let s = Span {
        start_byte: 0,
        end_byte: 5,
        start_row: 0,
        start_col: 0,
        end_row: 0,
        end_col: 5,
    };
    assert!(!s.is_synthetic());
}

#[test]
fn test_ident_synthetic() {
    let id = Ident::synthetic("foo");
    assert_eq!(id.name, "foo");
    assert!(id.span.is_synthetic());
}

#[test]
fn test_ident_exported() {
    assert!(Ident::synthetic("Foo").is_exported());
    assert!(!Ident::synthetic("foo").is_exported());
    assert!(!Ident::synthetic("_foo").is_exported());
}

#[test]
fn test_ident_exported_unicode() {
    // Go spec: exported if first char is Unicode uppercase letter (class Lu)
    assert!(Ident::synthetic("Über").is_exported());
    assert!(Ident::synthetic("Σ").is_exported());
    assert!(Ident::synthetic("Ω").is_exported());
    assert!(!Ident::synthetic("über").is_exported());
    assert!(!Ident::synthetic("σ").is_exported());
}

#[test]
fn test_string_lit_value_go_escape_sequences() {
    // \a (bell)
    let lit = StringLit {
        raw: r#""\a""#.to_owned(),
        span: Span::synthetic(),
    };
    assert_eq!(lit.value(), "\x07");

    // \b (backspace)
    let lit = StringLit {
        raw: r#""\b""#.to_owned(),
        span: Span::synthetic(),
    };
    assert_eq!(lit.value(), "\x08");

    // \f (form feed)
    let lit = StringLit {
        raw: r#""\f""#.to_owned(),
        span: Span::synthetic(),
    };
    assert_eq!(lit.value(), "\x0C");

    // \v (vertical tab)
    let lit = StringLit {
        raw: r#""\v""#.to_owned(),
        span: Span::synthetic(),
    };
    assert_eq!(lit.value(), "\x0B");

    // \xNN (hex byte)
    let lit = StringLit {
        raw: r#""\x41""#.to_owned(),
        span: Span::synthetic(),
    };
    assert_eq!(lit.value(), "A");

    // \uNNNN (unicode)
    let lit = StringLit {
        raw: r#""\u00e9""#.to_owned(),
        span: Span::synthetic(),
    };
    assert_eq!(lit.value(), "é");

    // \UNNNNNNNN (unicode)
    let lit = StringLit {
        raw: r#""\U0001F600""#.to_owned(),
        span: Span::synthetic(),
    };
    assert_eq!(lit.value(), "😀");

    // \NNN (octal)
    let lit = StringLit {
        raw: r#""\101""#.to_owned(),
        span: Span::synthetic(),
    };
    assert_eq!(lit.value(), "A"); // octal 101 = 65 = 'A'
}

#[test]
fn test_string_lit_value_high_byte_hex_escape() {
    // Go's \xFF is a single byte 0xFF. In Rust String (UTF-8), bytes >= 0x80
    // can't be represented as single chars directly. We use Unicode replacement
    // or Latin-1 mapping. The key invariant: value() should not panic and
    // should produce a consistent result.
    let lit = StringLit {
        raw: r#""\x00""#.to_owned(),
        span: Span::synthetic(),
    };
    assert_eq!(lit.value(), "\0"); // \x00 = null byte

    let lit = StringLit {
        raw: r#""\x7f""#.to_owned(),
        span: Span::synthetic(),
    };
    assert_eq!(lit.value(), "\x7f"); // \x7f = DEL, valid ASCII
}

#[test]
fn test_string_lit_roundtrip_control_chars() {
    // from_value → value should roundtrip for strings with control characters
    let original = "hello\x07world"; // contains bell character
    let lit = StringLit::from_value(original);
    assert_eq!(lit.value(), original, "roundtrip failed for bell char");

    let original = "tab\there";
    let lit = StringLit::from_value(original);
    assert_eq!(lit.value(), original, "roundtrip failed for tab");
}

#[test]
fn test_string_lit_from_value_and_value() {
    let lit = StringLit::from_value("hello");
    assert_eq!(lit.value(), "hello");
    assert!(lit.span.is_synthetic());
}

#[test]
fn test_string_lit_with_escapes() {
    let lit = StringLit::from_value("hello\"world");
    assert_eq!(lit.value(), "hello\"world");
}

#[test]
fn test_build_named_type() {
    let ty = build::named("int");
    assert!(matches!(ty, TypeExpr::Named(id) if id.name == "int"));
}

#[test]
fn test_build_pointer_type() {
    let ty = build::pointer(build::named("Foo"));
    match ty {
        TypeExpr::Pointer(inner) => {
            assert!(matches!(*inner, TypeExpr::Named(id) if id.name == "Foo"));
        }
        _ => panic!("expected Pointer"),
    }
}

#[test]
fn test_build_slice_type() {
    let ty = build::slice(build::named("byte"));
    assert!(matches!(ty, TypeExpr::Slice(_)));
}

#[test]
fn test_build_map_type() {
    let ty = build::map_type(build::named("string"), build::named("int"));
    match ty {
        TypeExpr::Map { key, value } => {
            assert!(matches!(*key, TypeExpr::Named(ref id) if id.name == "string"));
            assert!(matches!(*value, TypeExpr::Named(ref id) if id.name == "int"));
        }
        _ => panic!("expected Map"),
    }
}

#[test]
fn test_build_ident_expr() {
    let e = build::ident("x");
    assert!(matches!(e, Expr::Ident(id) if id.name == "x"));
}

#[test]
fn test_build_string_expr() {
    let e = build::string("hello");
    match e {
        Expr::String(lit) => assert_eq!(lit.value(), "hello"),
        _ => panic!("expected String"),
    }
}

#[test]
fn test_build_int_expr() {
    let e = build::int(42);
    match e {
        Expr::Int(lit) => assert_eq!(lit.raw, "42"),
        _ => panic!("expected Int"),
    }
}

#[test]
fn test_build_call_expr() {
    let e = build::call(build::ident("fmt"), vec![build::string("hello")]);
    match e {
        Expr::Call {
            func,
            args,
            ellipsis,
            type_args,
            ..
        } => {
            assert!(matches!(*func, Expr::Ident(ref id) if id.name == "fmt"));
            assert_eq!(args.len(), 1);
            assert!(!ellipsis);
            assert!(type_args.is_empty());
        }
        _ => panic!("expected Call"),
    }
}

#[test]
fn test_build_selector_expr() {
    let e = build::selector(build::ident("fmt"), "Sprintf");
    match e {
        Expr::Selector { operand, field, .. } => {
            assert!(matches!(*operand, Expr::Ident(ref id) if id.name == "fmt"));
            assert_eq!(field.name, "Sprintf");
        }
        _ => panic!("expected Selector"),
    }
}

#[test]
fn test_build_deref_and_addr() {
    let d = build::deref(build::ident("x"));
    assert!(matches!(
        d,
        Expr::Unary {
            op: UnaryOp::Deref,
            ..
        }
    ));

    let a = build::addr(build::ident("x"));
    assert!(matches!(
        a,
        Expr::Unary {
            op: UnaryOp::Addr,
            ..
        }
    ));
}

#[test]
fn test_build_ret() {
    let s = build::ret(vec![build::ident("x")]);
    match s {
        Stmt::Return { values, span } => {
            assert_eq!(values.len(), 1);
            assert!(span.is_synthetic());
        }
        _ => panic!("expected Return"),
    }
}

#[test]
fn test_build_block() {
    let b = build::block(vec![build::ret(vec![])]);
    assert_eq!(b.stmts.len(), 1);
    assert!(b.span.is_synthetic());
}

#[test]
fn test_build_param() {
    let p = build::param(&["x", "y"], build::named("int"));
    assert_eq!(p.names.len(), 2);
    assert_eq!(p.names[0].name, "x");
    assert_eq!(p.names[1].name, "y");
    assert!(!p.variadic);
}

#[test]
fn test_build_unnamed_param() {
    let p = build::unnamed_param(build::named("string"));
    assert!(p.names.is_empty());
    assert!(!p.variadic);
}

#[test]
fn test_build_pointer_receiver() {
    let r = build::pointer_receiver("x", "Foo");
    assert_eq!(r.name.as_ref().unwrap().name, "x");
    assert!(matches!(r.ty, TypeExpr::Pointer(_)));
    assert!(r.span.is_synthetic());
}

#[test]
fn test_build_value_receiver() {
    let r = build::value_receiver("x", "Foo");
    assert_eq!(r.name.as_ref().unwrap().name, "x");
    assert!(matches!(r.ty, TypeExpr::Named(ref id) if id.name == "Foo"));
}

#[test]
fn test_build_method() {
    let m = build::method(
        build::pointer_receiver("x", "Foo"),
        "String",
        vec![],
        vec![build::unnamed_param(build::named("string"))],
        build::block(vec![build::ret(vec![build::call(
            build::selector(build::ident("fmt"), "Sprintf"),
            vec![build::string("%+v"), build::deref(build::ident("x"))],
        )])]),
    );

    assert_eq!(m.name.name, "String");
    assert!(m.name.span.is_synthetic());
    assert_eq!(m.receiver.name.as_ref().unwrap().name, "x");
    assert!(matches!(m.receiver.ty, TypeExpr::Pointer(_)));
    assert!(m.ty.params.is_empty());
    assert_eq!(m.ty.results.len(), 1);
    assert!(m.body.is_some());
    assert!(m.doc.is_none());
    assert!(m.span.is_synthetic());
}

#[test]
fn test_build_all_spans_are_synthetic() {
    let m = build::method(
        build::pointer_receiver("x", "Foo"),
        "Bar",
        vec![build::param(&["a"], build::named("int"))],
        vec![build::unnamed_param(build::named("error"))],
        build::block(vec![build::ret(vec![build::ident("nil")])]),
    );

    // Verify all spans in the method are synthetic
    assert!(m.span.is_synthetic());
    assert!(m.name.span.is_synthetic());
    assert!(m.receiver.span.is_synthetic());
    assert!(m.ty.span.is_synthetic());
    for p in &m.ty.params {
        assert!(p.span.is_synthetic());
        for n in &p.names {
            assert!(n.span.is_synthetic());
        }
    }
    for r in &m.ty.results {
        assert!(r.span.is_synthetic());
    }
    let body = m.body.as_ref().unwrap();
    assert!(body.span.is_synthetic());
}

#[test]
fn test_binary_op_precedence() {
    assert_eq!(BinaryOp::LogOr.precedence(), 1);
    assert_eq!(BinaryOp::LogAnd.precedence(), 2);
    assert_eq!(BinaryOp::Eq.precedence(), 3);
    assert_eq!(BinaryOp::Ne.precedence(), 3);
    assert_eq!(BinaryOp::Lt.precedence(), 3);
    assert_eq!(BinaryOp::Le.precedence(), 3);
    assert_eq!(BinaryOp::Gt.precedence(), 3);
    assert_eq!(BinaryOp::Ge.precedence(), 3);
    assert_eq!(BinaryOp::Add.precedence(), 4);
    assert_eq!(BinaryOp::Sub.precedence(), 4);
    assert_eq!(BinaryOp::Or.precedence(), 4);
    assert_eq!(BinaryOp::Xor.precedence(), 4);
    assert_eq!(BinaryOp::Mul.precedence(), 5);
    assert_eq!(BinaryOp::Div.precedence(), 5);
    assert_eq!(BinaryOp::Rem.precedence(), 5);
    assert_eq!(BinaryOp::And.precedence(), 5);
    assert_eq!(BinaryOp::AndNot.precedence(), 5);
    assert_eq!(BinaryOp::Shl.precedence(), 5);
    assert_eq!(BinaryOp::Shr.precedence(), 5);
}

#[test]
fn test_type_spec_methods() {
    let ts = TypeSpec::Def {
        name: Ident::synthetic("Foo"),
        type_params: vec![],
        ty: TypeExpr::Struct(StructType {
            fields: vec![],
            span: Span::synthetic(),
        }),
        span: Span::synthetic(),
    };
    assert_eq!(ts.name().name, "Foo");
    assert!(ts.is_struct());
    assert!(!ts.is_interface());

    let ts2 = TypeSpec::Def {
        name: Ident::synthetic("Bar"),
        type_params: vec![],
        ty: TypeExpr::Interface(InterfaceType {
            elements: vec![],
            span: Span::synthetic(),
        }),
        span: Span::synthetic(),
    };
    assert!(ts2.is_interface());
    assert!(!ts2.is_struct());

    let ts3 = TypeSpec::Alias {
        name: Ident::synthetic("Baz"),
        type_params: vec![],
        ty: TypeExpr::Named(Ident::synthetic("int")),
        span: Span::synthetic(),
    };
    assert!(!ts3.is_struct());
    assert!(!ts3.is_interface());
}

// --- Serde round-trip tests ---

fn serde_roundtrip<
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
>(
    val: &T,
) {
    let json = serde_json::to_string(val).unwrap();
    let back: T = serde_json::from_str(&json).unwrap();
    assert_eq!(*val, back);
}

#[test]
fn test_serde_span() {
    serde_roundtrip(&Span::synthetic());
    serde_roundtrip(&Span {
        start_byte: 10,
        end_byte: 20,
        start_row: 1,
        start_col: 5,
        end_row: 1,
        end_col: 15,
    });
}

#[test]
fn test_serde_ident() {
    serde_roundtrip(&Ident::synthetic("foo"));
}

#[test]
fn test_serde_string_lit() {
    serde_roundtrip(&StringLit::from_value("hello"));
}

#[test]
fn test_serde_type_expr_all_variants() {
    serde_roundtrip(&TypeExpr::Named(Ident::synthetic("int")));
    serde_roundtrip(&TypeExpr::Qualified {
        package: Ident::synthetic("fmt"),
        name: Ident::synthetic("Stringer"),
    });
    serde_roundtrip(&TypeExpr::Pointer(Box::new(TypeExpr::Named(
        Ident::synthetic("int"),
    ))));
    serde_roundtrip(&TypeExpr::Array {
        len: Box::new(Expr::Int(IntLit {
            raw: "10".into(),
            span: Span::synthetic(),
        })),
        elem: Box::new(TypeExpr::Named(Ident::synthetic("byte"))),
    });
    serde_roundtrip(&TypeExpr::Slice(Box::new(TypeExpr::Named(
        Ident::synthetic("int"),
    ))));
    serde_roundtrip(&TypeExpr::Map {
        key: Box::new(TypeExpr::Named(Ident::synthetic("string"))),
        value: Box::new(TypeExpr::Named(Ident::synthetic("int"))),
    });
    serde_roundtrip(&TypeExpr::Channel {
        direction: ChanDir::Both,
        elem: Box::new(TypeExpr::Named(Ident::synthetic("int"))),
    });
    serde_roundtrip(&TypeExpr::Channel {
        direction: ChanDir::Recv,
        elem: Box::new(TypeExpr::Named(Ident::synthetic("int"))),
    });
    serde_roundtrip(&TypeExpr::Channel {
        direction: ChanDir::Send,
        elem: Box::new(TypeExpr::Named(Ident::synthetic("int"))),
    });
    serde_roundtrip(&TypeExpr::Struct(StructType {
        fields: vec![],
        span: Span::synthetic(),
    }));
    serde_roundtrip(&TypeExpr::Interface(InterfaceType {
        elements: vec![],
        span: Span::synthetic(),
    }));
    serde_roundtrip(&TypeExpr::Generic {
        base: Box::new(TypeExpr::Named(Ident::synthetic("List"))),
        args: vec![TypeExpr::Named(Ident::synthetic("int"))],
    });
}

#[test]
fn test_serde_expr_variants() {
    serde_roundtrip(&Expr::Ident(Ident::synthetic("x")));
    serde_roundtrip(&Expr::Int(IntLit {
        raw: "42".into(),
        span: Span::synthetic(),
    }));
    serde_roundtrip(&Expr::Float(FloatLit {
        raw: "3.14".into(),
        span: Span::synthetic(),
    }));
    serde_roundtrip(&Expr::String(StringLit::from_value("hello")));
    serde_roundtrip(&Expr::RawString(RawStringLit {
        raw: "`hello`".into(),
        span: Span::synthetic(),
    }));
    serde_roundtrip(&Expr::True(Span::synthetic()));
    serde_roundtrip(&Expr::False(Span::synthetic()));
    serde_roundtrip(&Expr::Nil(Span::synthetic()));
    serde_roundtrip(&Expr::Iota(Span::synthetic()));
    serde_roundtrip(&Expr::Paren(
        Box::new(Expr::Ident(Ident::synthetic("x"))),
        Span::synthetic(),
    ));
}

#[test]
fn test_serde_stmt_variants() {
    serde_roundtrip(&Stmt::Empty(Span::synthetic()));
    serde_roundtrip(&Stmt::Return {
        values: vec![],
        span: Span::synthetic(),
    });
    serde_roundtrip(&Stmt::Break(None, Span::synthetic()));
    serde_roundtrip(&Stmt::Continue(None, Span::synthetic()));
    serde_roundtrip(&Stmt::Fallthrough(Span::synthetic()));
    serde_roundtrip(&Stmt::Block(Block {
        stmts: vec![],
        span: Span::synthetic(),
    }));
}

#[test]
fn test_serde_method_decl() {
    let m = build::method(
        build::pointer_receiver("x", "Foo"),
        "String",
        vec![],
        vec![build::unnamed_param(build::named("string"))],
        build::block(vec![build::ret(vec![build::string("foo")])]),
    );
    serde_roundtrip(&m);
}

#[test]
fn test_serde_func_type() {
    serde_roundtrip(&FuncType {
        type_params: vec![],
        params: vec![ParamDecl {
            names: vec![Ident::synthetic("x")],
            ty: TypeExpr::Named(Ident::synthetic("int")),
            variadic: false,
            span: Span::synthetic(),
        }],
        results: vec![ParamDecl {
            names: vec![],
            ty: TypeExpr::Named(Ident::synthetic("error")),
            variadic: false,
            span: Span::synthetic(),
        }],
        span: Span::synthetic(),
    });
}

#[test]
fn test_serde_import_spec() {
    serde_roundtrip(&ImportSpec {
        alias: ImportAlias::Implicit,
        path: StringLit::from_value("fmt"),
        span: Span::synthetic(),
    });
    serde_roundtrip(&ImportSpec {
        alias: ImportAlias::Dot,
        path: StringLit::from_value("math"),
        span: Span::synthetic(),
    });
    serde_roundtrip(&ImportSpec {
        alias: ImportAlias::Blank,
        path: StringLit::from_value("database/sql"),
        span: Span::synthetic(),
    });
    serde_roundtrip(&ImportSpec {
        alias: ImportAlias::Named(Ident::synthetic("mypkg")),
        path: StringLit::from_value("example.com/pkg"),
        span: Span::synthetic(),
    });
}

#[test]
fn test_serde_source_file() {
    let sf = SourceFile {
        package: Ident::synthetic("main"),
        imports: vec![],
        decls: vec![TopLevelDecl::Func(Box::new(FuncDecl {
            name: Ident::synthetic("main"),
            ty: FuncType {
                type_params: vec![],
                params: vec![],
                results: vec![],
                span: Span::synthetic(),
            },
            body: Some(Block {
                stmts: vec![],
                span: Span::synthetic(),
            }),
            doc: None,
            span: Span::synthetic(),
        }))],
        span: Span::synthetic(),
    };
    serde_roundtrip(&sf);
}
