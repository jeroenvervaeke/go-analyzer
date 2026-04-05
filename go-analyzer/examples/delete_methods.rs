//! Delete all methods with a given name from a Go repository.
//!
//! Usage: cargo run --example delete_methods -- <path-to-go-repo> <method-name>
//!        cargo run --example delete_methods -- <path-to-go-repo> <method-name> --dry-run

use go_analyzer::Repo;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).map(|s| s.as_str()).unwrap_or(".");
    let method_name = args
        .get(2)
        .expect("usage: delete_methods <path> <method-name>");
    let dry_run = args.iter().any(|a| a == "--dry-run");

    let repo = Repo::load(path)?;

    let changes = repo.methods().named(method_name).delete();

    if changes.is_empty() {
        println!("No methods named '{method_name}' found.");
        return Ok(());
    }

    println!(
        "Deleting {} instance(s) of '{method_name}'.",
        changes.edit_count()
    );

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
