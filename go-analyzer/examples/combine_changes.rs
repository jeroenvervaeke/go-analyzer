//! Combine multiple independent changes into a single atomic commit.
//!
//! This example deletes all `String()` methods and renames all unexported
//! functions called `helper` to `internalHelper`, in one pass.
//!
//! Usage: cargo run --example combine_changes -- <path-to-go-repo>
//!        cargo run --example combine_changes -- <path-to-go-repo> --dry-run

use go_analyzer::{Changes, Repo};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).map(|s| s.as_str()).unwrap_or(".");
    let dry_run = args.iter().any(|a| a == "--dry-run");

    let repo = Repo::load(path)?;

    // Change 1: delete all String() methods
    let c1 = repo.structs().method("String").delete();
    println!("Delete String(): {} edit(s)", c1.edit_count());

    // Change 2: rename unexported 'helper' functions
    let c2 = repo
        .functions()
        .named("helper")
        .unexported()
        .rename("internalHelper");
    println!("Rename helper: {} edit(s)", c2.edit_count());

    let combined = Changes::combine([c1, c2]);

    if combined.is_empty() {
        println!("Nothing to do.");
        return Ok(());
    }

    let applied = repo.apply(combined);
    println!(
        "\n{} total edit(s) across {} file(s).",
        applied.edit_count(),
        applied.affected_files().len()
    );

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
