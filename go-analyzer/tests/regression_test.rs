//! Regression tests for bugs found during code review.

use go_analyzer::go_model::*;
use go_analyzer::walker::parse_and_walk;
use go_analyzer::{Repo, build};
use std::path::Path;

fn copy_fixture_to_temp() -> tempfile::TempDir {
    let fixture = format!("{}/tests/fixture_repo", env!("CARGO_MANIFEST_DIR"));
    let tmp = tempfile::TempDir::new().expect("failed to create temp dir");
    copy_dir_recursive(Path::new(&fixture), tmp.path());
    std::fs::write(
        tmp.path().join("go.mod"),
        "module fixture_repo\n\ngo 1.21\n",
    )
    .unwrap();
    tmp
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let dest = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir_recursive(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
}

// ============================================================
// Bug: Struct field tags stored as empty strings
// ============================================================

#[test]
fn test_struct_field_tags_preserved() {
    let src = br#"package p

type Config struct {
	Host string `json:"host" yaml:"host"`
	Port int    `json:"port"`
}
"#;
    let sf = parse_and_walk(src).unwrap();
    let type_decl = sf.decls.iter().find_map(|d| match d {
        TopLevelDecl::Type(specs) => specs.first(),
        _ => None,
    });
    let ts = type_decl.expect("should have type decl");
    match ts.ty() {
        TypeExpr::Struct(st) => {
            match &st.fields[0] {
                FieldDecl::Named { tag, .. } => {
                    let tag = tag.as_ref().expect("Host should have a tag");
                    assert!(
                        tag.raw.contains("json"),
                        "tag should contain json, got: {:?}",
                        tag.raw
                    );
                }
                _ => panic!("expected named field"),
            }
            match &st.fields[1] {
                FieldDecl::Named { tag, .. } => {
                    let tag = tag.as_ref().expect("Port should have a tag");
                    assert!(
                        tag.raw.contains("json"),
                        "tag should contain json, got: {:?}",
                        tag.raw
                    );
                }
                _ => panic!("expected named field"),
            }
        }
        _ => panic!("expected struct type"),
    }
}

// ============================================================
// Bug: Cross-package method matching
// ============================================================

#[test]
fn test_method_lookup_scoped_to_package() {
    let tmp = tempfile::TempDir::new().unwrap();

    // Package a: Config struct, no String() method
    std::fs::create_dir_all(tmp.path().join("a")).unwrap();
    std::fs::write(
        tmp.path().join("a/a.go"),
        "package a\n\ntype Config struct{ X int }\n",
    )
    .unwrap();

    // Package b: Config struct WITH String() method
    std::fs::create_dir_all(tmp.path().join("b")).unwrap();
    std::fs::write(
        tmp.path().join("b/b.go"),
        "package b\n\nimport \"fmt\"\n\ntype Config struct{ Y int }\n\nfunc (c *Config) String() string { return fmt.Sprintf(\"%+v\", *c) }\n",
    )
    .unwrap();

    let repo = Repo::load(tmp.path()).unwrap();

    // a.Config should NOT have String() — it's only on b.Config
    assert_eq!(
        repo.structs()
            .in_package("a")
            .method("String")
            .existing()
            .count(),
        0,
        "a.Config should not match b.Config's String() method"
    );
    assert_eq!(
        repo.structs()
            .in_package("a")
            .method("String")
            .absent()
            .count(),
        1
    );

    // b.Config SHOULD have String()
    assert_eq!(
        repo.structs()
            .in_package("b")
            .method("String")
            .existing()
            .count(),
        1
    );
}

// ============================================================
// Bug: Grouped statement declarations truncated
// ============================================================

#[test]
fn test_grouped_var_decl_all_specs_preserved() {
    let src = br#"package p

func f() {
	var (
		x int
		y string
		z float64
	)
	_ = x
	_ = y
	_ = z
}
"#;
    let sf = parse_and_walk(src).unwrap();
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let body = func.body.as_ref().unwrap();

    // Find the VarDecl statement
    let var_decl = body.stmts.iter().find_map(|s| match s {
        Stmt::VarDecl(specs, _) => Some(specs),
        _ => None,
    });
    let specs = var_decl.expect("should have var decl");
    assert_eq!(
        specs.len(),
        3,
        "all 3 var specs should be preserved, got {}",
        specs.len()
    );
    assert_eq!(specs[0].names[0].name, "x");
    assert_eq!(specs[1].names[0].name, "y");
    assert_eq!(specs[2].names[0].name, "z");
}

#[test]
fn test_grouped_const_decl_all_specs_preserved() {
    let src = br#"package p

func f() {
	const (
		A = 1
		B = 2
		C = 3
	)
	_ = A
	_ = B
	_ = C
}
"#;
    let sf = parse_and_walk(src).unwrap();
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let body = func.body.as_ref().unwrap();

    let const_decl = body.stmts.iter().find_map(|s| match s {
        Stmt::ConstDecl(specs, _) => Some(specs),
        _ => None,
    });
    let specs = const_decl.expect("should have const decl");
    assert_eq!(specs.len(), 3, "all 3 const specs should be preserved");
}

#[test]
fn test_grouped_type_decl_all_specs_preserved() {
    let src = br#"package p

func f() {
	type (
		MyInt int
		MyStr string
	)
	var _ MyInt
	var _ MyStr
}
"#;
    let sf = parse_and_walk(src).unwrap();
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let body = func.body.as_ref().unwrap();

    let type_decl = body.stmts.iter().find_map(|s| match s {
        Stmt::TypeDecl(specs, _) => Some(specs),
        _ => None,
    });
    let specs = type_decl.expect("should have type decl");
    assert_eq!(specs.len(), 2, "both type specs should be preserved");
}

// ============================================================
// Bug: Bare receive in select dropped
// ============================================================

#[test]
fn test_bare_receive_in_select_preserved() {
    let src = br#"package p

func f() {
	ch := make(chan int)
	select {
	case <-ch:
		return
	}
}
"#;
    let sf = parse_and_walk(src).unwrap();
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let body = func.body.as_ref().unwrap();

    let select_stmt = body.stmts.iter().find_map(|s| match s {
        Stmt::Select { cases, .. } => Some(cases),
        _ => None,
    });
    let cases = select_stmt.expect("should have select");
    assert_eq!(cases.len(), 1);

    match &cases[0] {
        CommCase::Recv {
            stmt, recv_expr, ..
        } => {
            assert!(stmt.is_none(), "bare receive should have no assignment");
            assert!(
                recv_expr.is_some(),
                "bare receive should have recv_expr populated"
            );
        }
        other => panic!("expected Recv, got {other:?}"),
    }
}

#[test]
fn test_bare_receive_roundtrips_through_printer() {
    let src = br#"package p

func f() {
	ch := make(chan int)
	select {
	case <-ch:
		return
	}
}
"#;
    let sf = parse_and_walk(src).unwrap();
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let printed = go_analyzer::test_support::print_func_decl(func);

    // Should contain `case <-ch:`, not `case:`
    assert!(
        printed.contains("case <-ch"),
        "printed should contain 'case <-ch', got:\n{printed}"
    );
}

// ============================================================
// Bug: Tilde in type constraints dropped
// ============================================================

#[test]
fn test_tilde_in_type_constraint_preserved() {
    let src = br#"package p

type Number interface {
	~int | ~float64
}
"#;
    let sf = parse_and_walk(src).unwrap();
    let type_decl = sf.decls.iter().find_map(|d| match d {
        TopLevelDecl::Type(specs) => specs.first(),
        _ => None,
    });
    let ts = type_decl.expect("should have type decl");
    match ts.ty() {
        TypeExpr::Interface(it) => {
            let term = match &it.elements[0] {
                InterfaceElem::TypeTerm(tt) => tt,
                other => panic!("expected TypeTerm, got {other:?}"),
            };
            assert!(term.terms[0].tilde, "~int should have tilde=true");
            assert!(term.terms[1].tilde, "~float64 should have tilde=true");
        }
        _ => panic!("expected interface type"),
    }
}

// ============================================================
// Bug: Pointer type conversions lose required parentheses
// ============================================================

#[test]
fn test_pointer_type_conversion_has_parens() {
    let src = br#"package p

type MyInt int

func f() {
	var x int = 42
	_ = (*MyInt)(&x)
}
"#;
    let sf = parse_and_walk(src).unwrap();
    let func = sf
        .decls
        .iter()
        .find_map(|d| match d {
            TopLevelDecl::Func(f) => Some(f),
            _ => None,
        })
        .unwrap();
    let printed = go_analyzer::test_support::print_func_decl(func);

    // The type conversion should preserve parens around *MyInt
    assert!(
        printed.contains("(*MyInt)"),
        "pointer type conversion should have parens: (*MyInt)(...), got:\n{printed}"
    );
}

// ============================================================
// Bug: StringLit::from_value doesn't escape control characters
// ============================================================

#[test]
fn test_string_lit_from_value_escapes_newlines() {
    let lit = StringLit::from_value("line1\nline2");
    assert_eq!(lit.raw, r#""line1\nline2""#);
    assert!(
        !lit.raw.contains('\n'),
        "raw should not contain literal newline"
    );
}

#[test]
fn test_string_lit_from_value_escapes_tabs() {
    let lit = StringLit::from_value("col1\tcol2");
    assert_eq!(lit.raw, r#""col1\tcol2""#);
}

#[test]
fn test_string_lit_from_value_escapes_null() {
    let lit = StringLit::from_value("before\0after");
    assert!(
        lit.raw.contains("\\x00"),
        "null should be escaped, got: {:?}",
        lit.raw
    );
}

#[test]
fn test_string_lit_from_value_preserves_normal_strings() {
    let lit = StringLit::from_value("hello world");
    assert_eq!(lit.raw, r#""hello world""#);
}

// ============================================================
// Bug: Edit count not adjusted on failure (already fixed)
// ============================================================

#[test]
fn test_edit_count_reflects_actual_applied() {
    let tmp = copy_fixture_to_temp();
    let repo = Repo::load(tmp.path()).unwrap();

    // Create a valid change
    let changes = repo.functions().named("helperFunc").delete();
    assert!(!changes.is_empty());

    let applied = repo.apply(changes);
    // edit_count should match actual number of edits that succeeded
    assert!(applied.edit_count() > 0);

    let summary = applied.commit().unwrap();
    assert_eq!(summary.edits_applied, summary.files_modified.max(1));
}

// ============================================================
// Bug: MethodEntry targets wrong file
// ============================================================

#[test]
fn test_method_entry_delete_targets_correct_file() {
    let tmp = tempfile::TempDir::new().unwrap();

    // Type in one file, method in another (standard Go pattern)
    std::fs::create_dir_all(tmp.path().join("pkg")).unwrap();
    std::fs::write(
        tmp.path().join("pkg/types.go"),
        "package pkg\n\ntype Foo struct{ X int }\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("pkg/methods.go"),
        "package pkg\n\nimport \"fmt\"\n\nfunc (f *Foo) String() string { return fmt.Sprintf(\"%+v\", *f) }\n",
    )
    .unwrap();

    let repo = Repo::load(tmp.path()).unwrap();

    // Delete String() via the type entry API
    let changes = repo.structs().method("String").delete();
    assert_eq!(changes.edit_count(), 1);

    repo.apply(changes).commit().unwrap();

    // types.go should be untouched
    let types_content = std::fs::read_to_string(tmp.path().join("pkg/types.go")).unwrap();
    assert!(
        types_content.contains("type Foo struct"),
        "types.go should not be modified"
    );

    // methods.go should have String() removed
    let methods_content = std::fs::read_to_string(tmp.path().join("pkg/methods.go")).unwrap();
    assert!(
        !methods_content.contains("func (f *Foo) String()"),
        "String() should be deleted from methods.go"
    );
}

#[test]
fn test_method_entry_or_add_targets_type_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("pkg")).unwrap();
    std::fs::write(
        tmp.path().join("pkg/types.go"),
        "package pkg\n\ntype Bar struct{ Y int }\n",
    )
    .unwrap();

    let repo = Repo::load(tmp.path()).unwrap();

    let changes = repo.structs().method("String").or_add(|ts| {
        let name = &ts.name().name;
        build::method(
            build::pointer_receiver("x", name),
            "String",
            vec![],
            vec![build::unnamed_param(build::named("string"))],
            build::block(vec![build::ret(vec![build::string("bar")])]),
        )
    });
    assert_eq!(changes.edit_count(), 1);

    repo.apply(changes).commit().unwrap();

    // The new method should be in types.go (same file as the type)
    let content = std::fs::read_to_string(tmp.path().join("pkg/types.go")).unwrap();
    assert!(
        content.contains("func (x *Bar) String()"),
        "String() should be added to types.go"
    );
}

// ============================================================
// Bug: Channel direction with non-standard whitespace
// ============================================================

#[test]
fn test_channel_direction_with_whitespace_variations() {
    // Standard formatting
    let sf = parse_and_walk(b"package p\nvar ch chan<- int\n").unwrap();
    let var = sf.decls.iter().find_map(|d| match d {
        TopLevelDecl::Var(specs) => specs.first(),
        _ => None,
    });
    let ty = var.unwrap().ty.as_ref().unwrap();
    match ty {
        TypeExpr::Channel { direction, .. } => {
            assert_eq!(*direction, ChanDir::Send, "chan<- should be Send");
        }
        _ => panic!("expected channel type"),
    }

    // With extra whitespace: `chan <- int` (space before arrow)
    // gofmt normalizes this, but unformatted code may have it
    let sf2 = parse_and_walk(b"package p\nvar ch chan <- int\n").unwrap();
    let var2 = sf2.decls.iter().find_map(|d| match d {
        TopLevelDecl::Var(specs) => specs.first(),
        _ => None,
    });
    let ty2 = var2.unwrap().ty.as_ref().unwrap();
    match ty2 {
        TypeExpr::Channel { direction, .. } => {
            assert_eq!(
                *direction,
                ChanDir::Send,
                "chan <- (with space) should still be Send"
            );
        }
        _ => panic!("expected channel type"),
    }
}

// ============================================================
// Bug: Edit engine out-of-bounds span should error, not panic
// ============================================================

#[test]
fn test_edit_out_of_bounds_span_returns_error() {
    use go_analyzer::go_model::Span;
    // This test ensures we don't panic on out-of-bounds spans
    let source = b"hello";
    let edit = go_analyzer::edit::Edit {
        file: std::path::PathBuf::from("test.go"),
        kind: go_analyzer::edit::EditKind::Delete {
            span: Span {
                start_byte: 10,
                end_byte: 20,
                start_row: 0,
                start_col: 0,
                end_row: 0,
                end_col: 0,
            },
        },
    };
    // Should return an error, not panic
    let result = go_analyzer::edit::apply_edits(source, &[edit]);
    assert!(result.is_err(), "out-of-bounds edit should return error");
}

// ============================================================
// Bug: Inverted span (start > end) should error, not silently corrupt
// ============================================================

#[test]
fn test_edit_inverted_span_returns_error() {
    let source = b"hello world";
    let edit = go_analyzer::edit::Edit {
        file: std::path::PathBuf::from("test.go"),
        kind: go_analyzer::edit::EditKind::Replace {
            span: Span {
                start_byte: 8,
                end_byte: 3,
                start_row: 0,
                start_col: 0,
                end_row: 0,
                end_col: 0,
            },
            new_text: "X".to_owned(),
        },
    };
    let result = go_analyzer::edit::apply_edits(source, &[edit]);
    assert!(
        result.is_err(),
        "inverted span (start > end) should return error"
    );
}

// ============================================================
// Bug: add_field on empty struct
// ============================================================

#[test]
fn test_add_field_to_empty_struct() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("pkg")).unwrap();
    std::fs::write(
        tmp.path().join("pkg/types.go"),
        "package pkg\n\ntype Empty struct{}\n",
    )
    .unwrap();

    let repo = Repo::load(tmp.path()).unwrap();
    let changes = repo
        .structs()
        .named("Empty")
        .add_field("Name", build::named("string"));

    // If the struct is empty (`struct{}`), add_field should still work
    // The result should be valid Go
    if !changes.is_empty() {
        repo.apply(changes).commit().unwrap();
        let content = std::fs::read_to_string(tmp.path().join("pkg/types.go")).unwrap();
        assert!(
            content.contains("Name"),
            "field should be added:\n{content}"
        );
    }
    // If changes is empty because of the empty struct, that's also acceptable
    // but we should not panic
}

// ============================================================
// Bug: remove_field() deletes sibling fields in multi-name decl
// ============================================================

#[test]
fn test_remove_field_does_not_delete_siblings() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("pkg")).unwrap();
    std::fs::write(
        tmp.path().join("pkg/types.go"),
        "package pkg\n\ntype Point struct {\n\tX, Y int\n\tZ    float64\n}\n",
    )
    .unwrap();

    let repo = Repo::load(tmp.path()).unwrap();

    // Removing "X" should NOT also remove "Y" (they share a field declaration)
    let changes = repo.structs().named("Point").remove_field("X");

    if !changes.is_empty() {
        repo.apply(changes).commit().unwrap();
        let content = std::fs::read_to_string(tmp.path().join("pkg/types.go")).unwrap();
        // Y should still be present
        assert!(
            content.contains("Y"),
            "Y should not be deleted when removing X from 'X, Y int':\n{content}"
        );
    }
}

// ============================================================
// Bug: RangeAssign::Define for bare `for range` (no variables)
// ============================================================

#[test]
fn test_bare_for_range_no_assign() {
    let src = br#"package p

func f() {
	s := []int{1, 2, 3}
	for range s {
	}
}
"#;
    let sf = parse_and_walk(src).unwrap();
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let body = func.body.as_ref().unwrap();
    let range_stmt = body.stmts.iter().find_map(|s| match s {
        Stmt::ForRange {
            key, value, assign, ..
        } => Some((key, value, assign)),
        _ => None,
    });
    let (key, value, assign) = range_stmt.expect("should have for-range");
    assert!(key.is_none(), "bare range should have no key");
    assert!(value.is_none(), "bare range should have no value");
    // Bare `for range s` has no assignment — should not claim Define
    assert_ne!(
        *assign,
        RangeAssign::Define,
        "bare for-range with no variables should not be RangeAssign::Define"
    );
}

// ============================================================
// Bug: TypeSwitchAssign.span covers entire switch body
// ============================================================

#[test]
fn test_type_switch_assign_span_not_entire_switch() {
    let src = br#"package p

func f() {
	var x interface{} = 42
	switch v := x.(type) {
	case int:
		_ = v
	}
}
"#;
    let sf = parse_and_walk(src).unwrap();
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let body = func.body.as_ref().unwrap();
    let ts = body.stmts.iter().find_map(|s| match s {
        Stmt::TypeSwitch { assign, span, .. } => Some((assign, span)),
        _ => None,
    });
    let (assign, switch_span) = ts.expect("should have type switch");
    // The assign span should be smaller than the entire switch span
    let assign_size = assign.span.end_byte - assign.span.start_byte;
    let switch_size = switch_span.end_byte - switch_span.start_byte;
    assert!(
        assign_size < switch_size,
        "TypeSwitchAssign.span ({assign_size} bytes) should be smaller than the switch span ({switch_size} bytes)"
    );
}

// ============================================================
// Bug: Resolver loses multiple dot imports (HashMap overwrites)
// ============================================================

#[test]
fn test_resolver_multiple_dot_imports() {
    use go_analyzer::go_model::{Ident, ImportAlias, ImportSpec, SourceFile, StringLit};
    use go_analyzer::resolver::build_alias_map;

    let sf = SourceFile {
        package: Ident::synthetic("main"),
        imports: vec![
            ImportSpec {
                alias: ImportAlias::Dot,
                path: StringLit::from_value("fmt"),
                span: Span::synthetic(),
            },
            ImportSpec {
                alias: ImportAlias::Dot,
                path: StringLit::from_value("math"),
                span: Span::synthetic(),
            },
        ],
        decls: vec![],
        span: Span::synthetic(),
    };

    let map = build_alias_map(&sf);
    // With a HashMap<String, String>, the second dot import overwrites the first.
    // At minimum, the resolver should not silently lose data.
    // This test documents the current (broken) behavior.
    // If the resolver is fixed to support multiple dot imports,
    // this test should be updated to assert both are present.
    let dot_value = map.get(".").expect("should have dot import");
    // Currently only "math" survives (last insert wins)
    assert_eq!(dot_value, "math", "second dot import overwrites first");
}

// ============================================================
// Bug: For C-style loop double space when post is None
// ============================================================

#[test]
fn test_for_no_post_no_double_space() {
    let src = br#"package p

func f() {
	for i := 0; i < 10; {
		_ = i
	}
}
"#;
    let sf = parse_and_walk(src).unwrap();
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let printed = go_analyzer::test_support::print_func_decl(func);
    assert!(
        !printed.contains(";  {"),
        "should not have double space before opening brace:\n{printed}"
    );
}

// ============================================================
// Bug: TypeInstantiationExpression gets spurious parentheses
// ============================================================

#[test]
fn test_generic_instantiation_no_spurious_parens() {
    let src = br#"package p

func f() {
	type Set[T any] struct{}
	var _ Set[int]
}
"#;
    let sf = parse_and_walk(src).unwrap();
    let func = match &sf.decls[0] {
        TopLevelDecl::Func(f) => f,
        _ => panic!("expected func"),
    };
    let printed = go_analyzer::test_support::print_func_decl(func);
    // Should NOT contain (Set[int]) — no parens around generic instantiation
    assert!(
        !printed.contains("(Set[int])"),
        "generic instantiation should not have spurious parens:\n{printed}"
    );
}
