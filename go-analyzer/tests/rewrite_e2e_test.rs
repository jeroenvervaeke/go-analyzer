use std::path::Path;
use std::process::Command;

use go_analyzer::{Changes, Repo, build};

/// Copy the fixture repo to a temp directory for mutation.
fn copy_fixture_to_temp() -> tempfile::TempDir {
    let fixture = format!("{}/tests/fixture_repo", env!("CARGO_MANIFEST_DIR"));
    let tmp = tempfile::TempDir::new().expect("failed to create temp dir");
    copy_dir_recursive(Path::new(&fixture), tmp.path());

    // Initialize go.mod so `go build` works
    let go_mod = "module fixture_repo\n\ngo 1.21\n".to_string();
    std::fs::write(tmp.path().join("go.mod"), go_mod).expect("failed to write go.mod");

    tmp
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("mkdir");
    for entry in std::fs::read_dir(src).expect("read_dir") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &dest);
        } else {
            std::fs::copy(&path, &dest).expect("copy");
        }
    }
}

fn go_build(dir: &Path) -> bool {
    Command::new("go")
        .arg("build")
        .arg("./...")
        .current_dir(dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .status()
        .is_ok_and(|s| s.success())
}

fn gofmt_check_dir(dir: &Path) -> Vec<String> {
    let output = Command::new("gofmt")
        .arg("-l")
        .arg(".")
        .current_dir(dir)
        .output()
        .expect("gofmt -l failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

// --- Delete test ---

#[test]
fn test_delete_string_methods() {
    let tmp = copy_fixture_to_temp();
    let repo = Repo::load(tmp.path()).expect("load");

    // Verify String() methods exist
    let before = repo.structs().method("String").existing().count();
    assert!(before > 0, "fixture should have String() methods");

    // Delete all String() methods
    let changes = repo.structs().method("String").delete();
    assert!(!changes.is_empty());

    repo.apply(changes).commit().expect("commit");

    // Verify no String() methods remain
    let repo2 = Repo::load(tmp.path()).expect("reload");
    let after = repo2.structs().method("String").existing().count();
    assert_eq!(after, 0, "all String() methods should be deleted");

    // Verify output compiles
    assert!(go_build(tmp.path()), "go build should succeed after delete");
}

// --- Add test ---

#[test]
fn test_add_string_methods() {
    let tmp = copy_fixture_to_temp();

    // First, delete existing String() methods so we can add them back
    {
        let repo = Repo::load(tmp.path()).expect("load");
        let changes = repo.structs().method("String").delete();
        repo.apply(changes).commit().expect("commit delete");
    }

    // Now add String() to all exported structs
    let repo = Repo::load(tmp.path()).expect("reload");
    let absent_before = repo.structs().exported().method("String").absent().count();
    assert!(absent_before > 0, "should have structs without String()");

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
    assert!(!changes.is_empty());

    repo.apply(changes).commit().expect("commit add");

    // Verify every exported struct now has String()
    let repo3 = Repo::load(tmp.path()).expect("reload2");
    let absent_after = repo3.structs().exported().method("String").absent().count();
    assert_eq!(absent_after, 0, "all exported structs should have String()");

    // Verify output compiles
    assert!(go_build(tmp.path()), "go build should succeed after add");

    // Verify gofmt -l reports no files needing formatting
    let unformatted = gofmt_check_dir(tmp.path());
    assert!(
        unformatted.is_empty(),
        "gofmt -l should report no files, got: {unformatted:?}"
    );
}

// --- Combine test ---

#[test]
fn test_combine_changes() {
    let tmp = copy_fixture_to_temp();
    let repo = Repo::load(tmp.path()).expect("load");

    // Change 1: delete String() methods
    let c1 = repo.structs().method("String").delete();
    let c1_count = c1.edit_count();

    // Change 2: rename helperFunc to internalHelper
    let c2 = repo
        .functions()
        .named("helperFunc")
        .rename("internalHelper");
    let c2_count = c2.edit_count();

    assert!(c1_count > 0);
    assert!(c2_count > 0);

    let combined = Changes::combine([c1, c2]);
    assert_eq!(combined.edit_count(), c1_count + c2_count);

    repo.apply(combined).commit().expect("commit combined");

    // Verify both changes applied
    let repo2 = Repo::load(tmp.path()).expect("reload");
    assert_eq!(
        repo2.structs().method("String").existing().count(),
        0,
        "String methods should be deleted"
    );
    assert_eq!(
        repo2.functions().named("internalHelper").count(),
        1,
        "helperFunc should be renamed"
    );
    assert_eq!(
        repo2.functions().named("helperFunc").count(),
        0,
        "old name should not exist"
    );

    assert!(
        go_build(tmp.path()),
        "go build should succeed after combined changes"
    );
}

// --- Dry run test ---

#[test]
fn test_dry_run_no_disk_modification() {
    let tmp = copy_fixture_to_temp();
    let repo = Repo::load(tmp.path()).expect("load");

    let changes = repo.structs().method("String").delete();
    assert!(!changes.is_empty());

    let applied = repo.apply(changes);
    let dry = applied.dry_run();

    // dry_run returns modified source but doesn't write
    assert!(!dry.is_empty());

    // Verify files on disk are unchanged — reload and check String() still exists
    let repo2 = Repo::load(tmp.path()).expect("reload");
    let still_there = repo2.structs().method("String").existing().count();
    assert!(still_there > 0, "dry_run should not modify files on disk");
}

// --- Preview test ---

#[test]
fn test_preview_before_commit() {
    let tmp = copy_fixture_to_temp();
    let repo = Repo::load(tmp.path()).expect("load");

    let changes = repo.functions().named("helperFunc").delete();
    let applied = repo.apply(changes);

    // preview returns self for chaining
    let applied = applied.preview();

    // Disk should not be modified yet
    let repo2 = Repo::load(tmp.path()).expect("reload");
    assert_eq!(
        repo2.functions().named("helperFunc").count(),
        1,
        "preview should not modify disk"
    );

    // Now commit
    applied.commit().expect("commit");

    let repo3 = Repo::load(tmp.path()).expect("reload2");
    assert_eq!(
        repo3.functions().named("helperFunc").count(),
        0,
        "commit should have deleted helperFunc"
    );
}
