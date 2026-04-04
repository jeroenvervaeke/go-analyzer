use go_analyzer::go_model::TopLevelDecl;
use go_analyzer::test_support::{print_func_decl, print_method_decl};
use go_analyzer::walker::parse_and_walk;
use std::io::Write;
use std::process::Command;

fn gofmt_check(src: &str) -> Result<(), String> {
    let mut child = Command::new("gofmt")
        .arg("-e")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("gofmt not found");
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(src.as_bytes()).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn roundtrip_test(src: &str) {
    let sf = parse_and_walk(src.as_bytes()).expect("walk failed");
    for decl in &sf.decls {
        match decl {
            TopLevelDecl::Func(f) => {
                let printed = print_func_decl(f);
                let wrapped = format!("package p\n\n{printed}\n");
                if let Err(e) = gofmt_check(&wrapped) {
                    panic!(
                        "gofmt rejected func {}:\n{wrapped}\nError: {e}",
                        f.name.name
                    );
                }
            }
            TopLevelDecl::Method(m) => {
                let printed = print_method_decl(m);
                let wrapped = format!("package p\n\n{printed}\n");
                if let Err(e) = gofmt_check(&wrapped) {
                    panic!(
                        "gofmt rejected method {}:\n{wrapped}\nError: {e}",
                        m.name.name
                    );
                }
            }
            _ => {}
        }
    }
}

#[test]
fn test_roundtrip_select() {
    roundtrip_test(
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
}

#[test]
fn test_roundtrip_nested_if() {
    roundtrip_test(
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
}

#[test]
fn test_roundtrip_switch_in_for() {
    roundtrip_test(
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
}

#[test]
fn test_roundtrip_func_lit() {
    roundtrip_test(
        r#"package p

func f() {
	g := func(x int) int {
		return x + 1
	}
	_ = g
}
"#,
    );
}
