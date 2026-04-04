use go_analyzer::walker::{parse_and_walk, parse_has_error};
use std::process::Command;

#[test]
fn corpus_test_goroot_src() {
    // Run in a thread with a larger stack to handle deeply nested Go files
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(corpus_test_inner)
        .unwrap()
        .join()
        .unwrap();
}

fn corpus_test_inner() {
    let goroot = std::env::var("GOROOT")
        .or_else(|_| {
            Command::new("go")
                .arg("env")
                .arg("GOROOT")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .expect("GOROOT not found");

    let src_dir = std::path::Path::new(&goroot).join("src");
    assert!(
        src_dir.exists(),
        "GOROOT/src not found at {}",
        src_dir.display()
    );

    let mut files_processed = 0usize;
    let mut files_skipped = 0usize;
    let mut walker_errors = 0usize;
    let mut error_details: Vec<String> = Vec::new();

    for path in walkdir(&src_dir) {
        if path.extension().is_none_or(|e| e != "go") {
            continue;
        }
        // Skip testdata — intentionally malformed Go
        if path.components().any(|c| c.as_os_str() == "testdata") {
            continue;
        }

        let src = match std::fs::read(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if parse_has_error(&src) {
            files_skipped += 1;
            continue;
        }

        files_processed += 1;

        if let Err(e) = parse_and_walk(&src) {
            walker_errors += 1;
            if error_details.len() < 20 {
                error_details.push(format!("{}: {e}", path.display()));
            }
        }
    }

    println!("\n=== Corpus Test Summary ===");
    println!("Files processed:  {files_processed}");
    println!("Files skipped:    {files_skipped}");
    println!("Walker errors:    {walker_errors}");

    if !error_details.is_empty() {
        println!("\nErrors:");
        for d in &error_details {
            println!("  {d}");
        }
    }

    assert_eq!(walker_errors, 0, "Walker had {walker_errors} errors");
}

fn walkdir(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    walkdir_inner(dir, &mut result);
    result
}

fn walkdir_inner(dir: &std::path::Path, result: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.is_dir() {
            walkdir_inner(&path, result);
        } else {
            result.push(path);
        }
    }
}
