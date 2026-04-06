//! Parse a single Go file, walk it into the go-model AST, and print
//! every function/method declaration back to source.
//!
//! Usage: cargo run --example walk_and_print -- <file.go>

use go_analyzer::go_model::TopLevelDecl;
use go_analyzer::test_support::{print_func_decl, print_method_decl};
use go_analyzer::walker::parse_and_walk;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .expect("usage: walk_and_print <file.go>");

    let src = std::fs::read(&path)?;
    let source_file = parse_and_walk(&src)?;

    println!("package {}", source_file.package.name);
    println!("imports: {}", source_file.imports.len());
    println!("declarations: {}", source_file.decls.len());
    println!();

    for decl in &source_file.decls {
        match decl {
            TopLevelDecl::Func(f) => {
                println!("--- func {} ---", f.name.name);
                println!("{}", print_func_decl(f));
                println!();
            }
            TopLevelDecl::Method(m) => {
                println!("--- method {} ---", m.name.name);
                println!("{}", print_method_decl(m));
                println!();
            }
            TopLevelDecl::Type(specs) => {
                for spec in specs {
                    println!("--- type {} ---", spec.name().name);
                }
            }
            TopLevelDecl::Var(specs) => {
                for spec in specs {
                    let names: Vec<_> = spec.names.iter().map(|n| n.name.as_str()).collect();
                    println!("--- var {} ---", names.join(", "));
                }
            }
            TopLevelDecl::Const(specs) => {
                for spec in specs {
                    let names: Vec<_> = spec.names.iter().map(|n| n.name.as_str()).collect();
                    println!("--- const {} ---", names.join(", "));
                }
            }
        }
    }

    Ok(())
}
