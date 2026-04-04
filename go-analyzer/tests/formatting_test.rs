//! Tests that validate our printer produces correctly formatted Go code
//! matching gofmt conventions. These tests compare against expected output
//! strings and optionally validate with the real gofmt binary.

use go_analyzer::test_support::print_func_decl;
use go_analyzer::walker::parse_and_walk;
use go_model::*;
use std::io::Write;
use std::process::Command;

/// Parse Go source → walk → print the first function → compare to expected.
fn roundtrip_func(src: &str) -> String {
    let sf = parse_and_walk(src.as_bytes()).expect("walk failed");
    let func = sf
        .decls
        .iter()
        .find_map(|d| match d {
            TopLevelDecl::Func(f) => Some(f),
            _ => None,
        })
        .expect("no function found");
    print_func_decl(func)
}

/// Check that gofmt accepts the source (wrapped in a package).
fn assert_gofmt_accepts(go_src: &str) {
    let full = format!("package p\n\n{go_src}\n");
    let mut child = Command::new("gofmt")
        .arg("-e")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("gofmt not found");
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(full.as_bytes()).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "gofmt rejected:\n---\n{full}\n---\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Check that gofmt produces the exact same output (idempotent formatting).
fn assert_gofmt_idempotent(go_src: &str) {
    let full = format!("package p\n\n{go_src}\n");
    let mut child = Command::new("gofmt")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("gofmt not found");
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(full.as_bytes()).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "gofmt failed");
    let formatted = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        full, formatted,
        "Output is not gofmt-idempotent.\n--- ours ---\n{full}\n--- gofmt ---\n{formatted}"
    );
}

// ==================== Indentation depth tests ====================

#[test]
fn test_nested_if_indentation() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	if true {
		if false {
			return
		}
	}
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    assert!(
        printed.contains("\t\tif false"),
        "inner if should be at 2 tabs:\n{printed}"
    );
    assert!(
        printed.contains("\t\t\treturn"),
        "inner return should be at 3 tabs:\n{printed}"
    );
}

#[test]
fn test_deeply_nested_blocks() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	if true {
		for i := 0; i < 10; i++ {
			if i > 5 {
				go func() {
					return
				}()
			}
		}
	}
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    // The innermost `return` should be at 5 tabs
    assert!(
        printed.contains("\t\t\t\t\treturn"),
        "deeply nested return should be at 5 tabs:\n{printed}"
    );
}

#[test]
fn test_switch_case_indentation() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	switch x := 1; x {
	case 1:
		return
	case 2, 3:
		break
	default:
		return
	}
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    // case labels at 1 tab (same as switch body level)
    assert!(
        printed.contains("\tcase 1:"),
        "case should be at 1 tab:\n{printed}"
    );
    // case body at 2 tabs
    assert!(
        printed.contains("\t\treturn"),
        "case body should be at 2 tabs:\n{printed}"
    );
}

#[test]
fn test_switch_inside_for_indentation() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	for i := 0; i < 10; i++ {
		switch i {
		case 0:
			break
		default:
			continue
		}
	}
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    // switch cases at 2 tabs (for body = 2, case label = same level)
    assert!(
        printed.contains("\t\tcase 0:"),
        "case inside for should be at 2 tabs:\n{printed}"
    );
    // case body at 3 tabs
    assert!(
        printed.contains("\t\t\tbreak"),
        "case body inside for should be at 3 tabs:\n{printed}"
    );
}

#[test]
fn test_select_indentation() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	ch := make(chan int)
	select {
	case x := <-ch:
		_ = x
	default:
		return
	}
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    assert!(
        printed.contains("\tcase ") || printed.contains("\tdefault:"),
        "select case labels should be at 1 tab:\n{printed}"
    );
}

#[test]
fn test_type_switch_indentation() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	var x interface{} = 42
	switch v := x.(type) {
	case int:
		_ = v
	case string:
		_ = v
	default:
		return
	}
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
}

// ==================== Empty block tests ====================

#[test]
fn test_empty_func_body() {
    let printed = roundtrip_func("package p\nfunc f() {}\n");
    assert_eq!(printed, "func f() {}");
    assert_gofmt_idempotent(&printed);
}

#[test]
fn test_empty_for_body() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	for {
	}
}
"#,
    );
    assert_gofmt_accepts(&printed);
}

// ==================== If-else formatting ====================

#[test]
fn test_if_else_on_same_line() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	if true {
		return
	} else {
		return
	}
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    assert!(
        printed.contains("} else {"),
        "'}} else {{' should be on one line:\n{printed}"
    );
}

#[test]
fn test_if_else_if_chain() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	x := 1
	if x == 1 {
		return
	} else if x == 2 {
		return
	} else {
		return
	}
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    assert!(
        printed.contains("} else if"),
        "else if should be on same line as closing brace:\n{printed}"
    );
}

// ==================== For loop formatting ====================

#[test]
fn test_for_range() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	s := []int{1, 2, 3}
	for k, v := range s {
		_ = k
		_ = v
	}
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
}

#[test]
fn test_c_style_for() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	for i := 0; i < 10; i++ {
		_ = i
	}
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
}

#[test]
fn test_infinite_for() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	for {
		break
	}
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
}

// ==================== Expression formatting ====================

#[test]
fn test_composite_literal() {
    let printed = roundtrip_func(
        r#"package p

type Point struct{ X, Y int }

func f() {
	_ = Point{X: 1, Y: 2}
}
"#,
    );
    assert_gofmt_accepts(&printed);
}

#[test]
fn test_function_literal() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	g := func(x int) int {
		return x + 1
	}
	_ = g
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
}

// ==================== Operator spacing ====================

#[test]
fn test_binary_operator_spacing() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	_ = 1 + 2
	_ = 3 * 4
	_ = true && false
	_ = 1 == 2
	_ = 1 << 3
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    assert!(printed.contains("1 + 2"), "space around +");
    assert!(printed.contains("3 * 4"), "space around *");
    assert!(printed.contains("true && false"), "space around &&");
    assert!(printed.contains("1 == 2"), "space around ==");
    assert!(printed.contains("1 << 3"), "space around <<");
}

#[test]
fn test_unary_operator_no_space() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	x := 1
	_ = -x
	_ = !true
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert!(printed.contains("-x"), "no space after unary -");
    assert!(printed.contains("!true"), "no space after !");
}

// ==================== Statement formatting ====================

#[test]
fn test_send_receive() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	ch := make(chan int)
	ch <- 42
	x := <-ch
	_ = x
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    assert!(printed.contains("ch <- 42"), "space around <- in send");
}

#[test]
fn test_defer_go() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	go func() {}()
	defer func() {}()
}
"#,
    );
    assert_gofmt_accepts(&printed);
}

#[test]
fn test_return_multiple_values() {
    let printed = roundtrip_func(
        r#"package p
func f() (int, error) {
	return 0, nil
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
}

// ==================== Declaration formatting ====================

#[test]
fn test_method_with_pointer_receiver() {
    let src = r#"package p
type Foo struct{}
func (f *Foo) Bar() string {
	return "bar"
}
"#;
    let sf = parse_and_walk(src.as_bytes()).expect("walk");
    let method = sf
        .decls
        .iter()
        .find_map(|d| match d {
            TopLevelDecl::Method(m) => Some(m),
            _ => None,
        })
        .expect("no method");
    let printed = go_analyzer::test_support::print_method_decl(method);
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    assert!(
        printed.contains("(f *Foo)"),
        "receiver should be (f *Foo):\n{printed}"
    );
}

#[test]
fn test_variadic_function() {
    let printed = roundtrip_func(
        r#"package p
func f(args ...int) int {
	return 0
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    assert!(printed.contains("...int"), "variadic should use ...");
}

#[test]
fn test_generic_function() {
    let printed = roundtrip_func(
        r#"package p
func f[T any](x T) T {
	return x
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    assert!(printed.contains("[T any]"), "generic params");
}

#[test]
fn test_multiple_return_types() {
    let printed = roundtrip_func(
        r#"package p
func f() (string, error) {
	return "", nil
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
    assert!(
        printed.contains("(string, error)"),
        "multiple returns in parens"
    );
}

// ==================== Channel type formatting ====================

#[test]
fn test_channel_directions() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	var a chan int
	var b <-chan int
	var c chan<- int
	_ = a
	_ = b
	_ = c
}
"#,
    );
    assert_gofmt_accepts(&printed);
}

// ==================== Slice expression formatting ====================

#[test]
fn test_slice_expressions() {
    let printed = roundtrip_func(
        r#"package p
func f() {
	s := []int{1, 2, 3, 4, 5}
	_ = s[1:3]
	_ = s[:3]
	_ = s[1:]
	_ = s[1:3:5]
}
"#,
    );
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);
}

// ==================== Build module output formatting ====================

#[test]
fn test_build_method_formatting() {
    use go_analyzer::build;

    let method = build::method(
        build::pointer_receiver("x", "MyType"),
        "String",
        vec![],
        vec![build::unnamed_param(build::named("string"))],
        build::block(vec![build::ret(vec![build::call(
            build::selector(build::ident("fmt"), "Sprintf"),
            vec![build::string("%+v"), build::deref(build::ident("x"))],
        )])]),
    );

    let printed = go_analyzer::test_support::print_method_decl(&method);
    assert_gofmt_accepts(&printed);
    assert_gofmt_idempotent(&printed);

    // Verify correct indentation
    assert!(
        printed.contains("\treturn"),
        "return should be indented one tab:\n{printed}"
    );
}
