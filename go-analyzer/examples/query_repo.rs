//! Read-only queries against a Go repository.
//!
//! Usage: cargo run --example query_repo -- <path-to-go-repo>

use go_analyzer::Repo;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let repo = Repo::load(&path)?;

    println!("=== Structs ===");
    repo.structs().for_each(|t| {
        println!("  {}", t.name().name);
    });

    println!("\n=== Exported functions ===");
    repo.functions().exported().for_each(|f| {
        println!("  {}", f.name.name);
    });

    println!("\n=== Methods per type ===");
    for si in repo.structs().collect() {
        let name = &si.item.name().name;
        let count = repo.methods().on_type(name).count();
        println!("  {name}: {count} method(s)");
    }

    println!("\n=== Structs missing String() ===");
    let missing = repo.structs().exported().method("String").absent().count();
    println!("  {missing} exported struct(s) without String()");

    Ok(())
}
