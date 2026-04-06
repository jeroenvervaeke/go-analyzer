use go_model::*;

use super::parse_and_walk;

fn walk_fixture(name: &str) -> SourceFile {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    parse_and_walk(&src).unwrap_or_else(|e| panic!("walk failed for {name}: {e}"))
}

// --- Import tests ---

#[test]
fn test_walk_imports() {
    let sf = walk_fixture("imports.go");
    assert_eq!(sf.package.name, "fixtures");
    assert_eq!(sf.imports.len(), 4);

    // Bare import
    assert!(matches!(sf.imports[0].alias, ImportAlias::Implicit));
    assert!(sf.imports[0].path.raw.contains("fmt"));

    // Dot import
    assert!(matches!(sf.imports[1].alias, ImportAlias::Dot));
    assert!(sf.imports[1].path.raw.contains("math"));

    // Blank import
    assert!(matches!(sf.imports[2].alias, ImportAlias::Blank));
    assert!(sf.imports[2].path.raw.contains("database/sql"));

    // Named import
    match &sf.imports[3].alias {
        ImportAlias::Named(id) => assert_eq!(id.name, "mypkg"),
        other => panic!("expected Named, got {other:?}"),
    }
}

// --- Function tests ---

#[test]
fn test_walk_functions() {
    let sf = walk_fixture("functions.go");
    let funcs: Vec<_> = sf
        .decls
        .iter()
        .filter_map(|d| match d {
            TopLevelDecl::Func(f) => Some(f),
            _ => None,
        })
        .collect();

    assert_eq!(funcs.len(), 4);

    // Simple
    assert_eq!(funcs[0].name.name, "Simple");
    assert!(funcs[0].ty.params.is_empty());
    assert!(funcs[0].ty.results.is_empty());

    // WithParams
    assert_eq!(funcs[1].name.name, "WithParams");
    assert_eq!(funcs[1].ty.params.len(), 2);
    assert_eq!(funcs[1].ty.results.len(), 2);

    // Variadic
    assert_eq!(funcs[2].name.name, "Variadic");
    assert!(funcs[2].ty.params.last().unwrap().variadic);

    // Generic
    assert_eq!(funcs[3].name.name, "Generic");
    assert!(!funcs[3].ty.type_params.is_empty());
}

// --- Method tests ---

#[test]
fn test_walk_methods() {
    let sf = walk_fixture("methods.go");
    let methods: Vec<_> = sf
        .decls
        .iter()
        .filter_map(|d| match d {
            TopLevelDecl::Method(m) => Some(m),
            _ => None,
        })
        .collect();

    assert_eq!(methods.len(), 2);

    // Pointer receiver
    let pm = &methods[0];
    assert_eq!(pm.name.name, "PointerMethod");
    assert!(matches!(pm.receiver.ty, TypeExpr::Pointer(_)));
    assert_eq!(pm.receiver.name.as_ref().unwrap().name, "f");

    // Value receiver
    let vm = &methods[1];
    assert_eq!(vm.name.name, "ValueMethod");
    assert!(matches!(vm.receiver.ty, TypeExpr::Named(ref id) if id.name == "Foo"));
}

// --- Type tests ---

#[test]
fn test_walk_types() {
    let sf = walk_fixture("types.go");
    let type_decls: Vec<_> = sf
        .decls
        .iter()
        .filter_map(|d| match d {
            TopLevelDecl::Type(specs) => Some(specs),
            _ => None,
        })
        .collect();

    // 5 type declarations: struct, interface, alias, newtype, generic
    assert_eq!(type_decls.len(), 5);

    // Struct
    let s = &type_decls[0][0];
    assert_eq!(s.name().name, "MyStruct");
    assert!(s.is_struct());

    // Interface
    let i = &type_decls[1][0];
    assert_eq!(i.name().name, "MyInterface");
    assert!(i.is_interface());

    // Alias
    let a = &type_decls[2][0];
    assert_eq!(a.name().name, "MyAlias");
    assert!(matches!(a, TypeSpec::Alias { .. }));

    // Newtype
    let n = &type_decls[3][0];
    assert_eq!(n.name().name, "MyNewtype");
    assert!(matches!(n, TypeSpec::Def { .. }));

    // Generic type
    let g = &type_decls[4][0];
    assert_eq!(g.name().name, "GenericType");
}

// --- Statement tests ---

#[test]
fn test_walk_statements() {
    let sf = walk_fixture("statements.go");
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let stmts = &func.body.as_ref().unwrap().stmts;
    assert!(!stmts.is_empty(), "should have statements");

    // Check we have various statement types
    let has_var = stmts.iter().any(|s| matches!(s, Stmt::VarDecl(..)));
    let has_const = stmts.iter().any(|s| matches!(s, Stmt::ConstDecl(..)));
    let has_short = stmts.iter().any(|s| matches!(s, Stmt::ShortVarDecl { .. }));
    let has_assign = stmts.iter().any(|s| matches!(s, Stmt::Assign { .. }));
    let has_inc = stmts.iter().any(|s| matches!(s, Stmt::Inc(..)));
    let has_dec = stmts.iter().any(|s| matches!(s, Stmt::Dec(..)));
    let has_if = stmts.iter().any(|s| matches!(s, Stmt::If { .. }));
    let has_for = stmts.iter().any(|s| matches!(s, Stmt::For { .. }));
    let has_for_range = stmts.iter().any(|s| matches!(s, Stmt::ForRange { .. }));
    let has_switch = stmts.iter().any(|s| matches!(s, Stmt::Switch { .. }));
    let has_return = stmts.iter().any(|s| matches!(s, Stmt::Return { .. }));

    assert!(has_var, "missing var");
    assert!(has_const, "missing const");
    assert!(has_short, "missing short var decl");
    assert!(has_assign, "missing assign");
    assert!(has_inc, "missing inc");
    assert!(has_dec, "missing dec");
    assert!(has_if, "missing if");
    assert!(has_for, "missing for");
    assert!(has_for_range, "missing for range");
    assert!(has_switch, "missing switch");
    assert!(has_return, "missing return");
}

#[test]
fn test_walk_for_range_key_value() {
    let sf = walk_fixture("statements.go");
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let stmts = &func.body.as_ref().unwrap().stmts;
    let range_stmt = stmts
        .iter()
        .find(|s| matches!(s, Stmt::ForRange { .. }))
        .unwrap();
    match range_stmt {
        Stmt::ForRange {
            key, value, assign, ..
        } => {
            assert!(key.is_some(), "range should have key");
            assert!(value.is_some(), "range should have value");
            assert_eq!(*assign, RangeAssign::Define);
        }
        _ => unreachable!(),
    }
}

// --- Expression tests ---

#[test]
fn test_walk_expressions() {
    let sf = walk_fixture("expressions.go");
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let stmts = &func.body.as_ref().unwrap().stmts;
    assert!(!stmts.is_empty(), "should have statements");
    // If we got here without walker errors, all expressions parsed
}

// --- Channel tests ---

#[test]
fn test_walk_channels() {
    let sf = walk_fixture("channels.go");
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let stmts = &func.body.as_ref().unwrap().stmts;

    // Find send statement: ch <- 42
    let has_send = stmts.iter().any(|s| matches!(s, Stmt::Send { .. }));
    assert!(has_send, "missing send statement");

    // Check var decls for channel direction types
    let var_stmts: Vec<_> = stmts
        .iter()
        .filter_map(|s| match s {
            Stmt::VarDecl(vs, _) => Some(vs),
            _ => None,
        })
        .collect();

    // recv channel
    if let Some(recv_specs) = var_stmts.first()
        && let Some(recv_var) = recv_specs.first()
        && let Some(TypeExpr::Channel { direction, .. }) = &recv_var.ty
    {
        assert_eq!(*direction, ChanDir::Recv);
    }

    // send channel
    if let Some(send_specs) = var_stmts.get(1)
        && let Some(send_var) = send_specs.first()
        && let Some(TypeExpr::Channel { direction, .. }) = &send_var.ty
    {
        assert_eq!(*direction, ChanDir::Send);
    }
}

// --- No synthetic spans in walker output ---

#[test]
fn test_walker_no_synthetic_spans() {
    let sf = walk_fixture("functions.go");
    // Package ident should have a real span
    assert!(!sf.package.span.is_synthetic());
    assert!(!sf.span.is_synthetic());

    // Check first function's span is not synthetic
    match &sf.decls[0] {
        TopLevelDecl::Func(f) => {
            assert!(!f.span.is_synthetic());
            assert!(!f.name.span.is_synthetic());
        }
        _ => panic!("expected func"),
    }
}
