//! Add `String() string` to all exported structs that don't have one.
//!
//! Usage: cargo run --example add_string_methods -- <path-to-go-repo>
//!        cargo run --example add_string_methods -- <path-to-go-repo> --dry-run

use go_analyzer::{Repo, build};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).map(|s| s.as_str()).unwrap_or(".");
    let dry_run = args.iter().any(|a| a == "--dry-run");

    let repo = Repo::load(path)?;

    let changes = repo.structs().exported().method("String").or_add(|ts| {
        let name = &ts.name().name;
        build::method(
            build::pointer_receiver("x", name),
            "String",
            vec![],
            vec![build::unnamed_param(build::named("string"))],
            build::block(vec![build::ret(vec![build::call(
                build::selector(build::ident("fmt"), "Sprintf"),
                vec![build::string("%+v"), build::deref(build::ident("x"))],
            )])]),
        )
    });

    if changes.is_empty() {
        println!("All exported structs already have String().");
        return Ok(());
    }

    println!("{} edit(s) to apply.", changes.edit_count());

    let applied = repo.apply(changes);

    if dry_run {
        applied.preview();
        println!("(dry run — no files modified)");
    } else {
        let summary = applied.commit()?;
        println!(
            "Done: {} edit(s) across {} file(s).",
            summary.edits_applied, summary.files_modified
        );
    }

    Ok(())
}
