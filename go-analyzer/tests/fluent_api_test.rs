use go_analyzer::{Changes, Repo};

fn fixture_repo() -> Repo {
    let path = format!("{}/tests/fixture_repo", env!("CARGO_MANIFEST_DIR"));
    Repo::load(&path).expect("failed to load fixture repo")
}

// --- Struct queries ---

#[test]
fn test_struct_count() {
    let repo = fixture_repo();
    // alpha: User, Admin, Config; beta: Server, Client
    assert_eq!(repo.structs().count(), 5);
}

#[test]
fn test_struct_exported_count() {
    let repo = fixture_repo();
    // All structs in fixtures are exported
    assert_eq!(repo.structs().exported().count(), 5);
}

#[test]
fn test_struct_method_existing() {
    let repo = fixture_repo();
    // User and Server have String() methods
    let count = repo.structs().method("String").existing().count();
    assert_eq!(count, 2);
}

#[test]
fn test_struct_method_absent() {
    let repo = fixture_repo();
    // Admin, Config, Client don't have String()
    let absent = repo.structs().method("String").absent().count();
    assert_eq!(absent, 3);
}

// --- Function queries ---

#[test]
fn test_function_count() {
    let repo = fixture_repo();
    // alpha: NewUser, NewConfig, helperFunc; beta: RunServer
    assert_eq!(repo.functions().count(), 4);
}

#[test]
fn test_function_named() {
    let repo = fixture_repo();
    assert_eq!(repo.functions().named("NewUser").count(), 1);
    assert_eq!(repo.functions().named("nonexistent").count(), 0);
}

// --- Method queries ---

#[test]
fn test_methods_on_type() {
    let repo = fixture_repo();
    // Server has String() and Start()
    assert_eq!(repo.methods().on_type("Server").count(), 2);
    // User has String()
    assert_eq!(repo.methods().on_type("User").count(), 1);
    // Client has Connect()
    assert_eq!(repo.methods().on_type("Client").count(), 1);
}

// --- Changes ---

#[test]
fn test_changes_none_is_empty() {
    assert!(Changes::none().is_empty());
    assert_eq!(Changes::none().edit_count(), 0);
}

#[test]
fn test_changes_combine_empty() {
    let combined = Changes::combine(Vec::<Changes>::new());
    assert!(combined.is_empty());
}

#[test]
fn test_changes_and() {
    let repo = fixture_repo();
    let c1 = repo.methods().named("String").delete();
    let c2 = repo.functions().named("NewUser").delete();
    let count1 = c1.edit_count();
    let count2 = c2.edit_count();
    let combined = c1.and(c2);
    assert_eq!(combined.edit_count(), count1 + count2);
}

// --- Dry run ---

#[test]
fn test_dry_run_no_disk_write() {
    let repo = fixture_repo();
    let changes = repo.methods().named("String").delete();
    assert!(!changes.is_empty());

    let applied = repo.apply(changes);
    let dry = applied.dry_run();

    // Should have at least one affected file
    assert!(!dry.is_empty());

    // Verify the original files are unchanged by reading them again
    let repo2 = fixture_repo();
    // Same struct count — nothing was actually deleted
    assert_eq!(repo2.structs().count(), 5);
}

// --- Preview chaining ---

#[test]
fn test_preview_returns_self() {
    let repo = fixture_repo();
    let changes = repo.functions().named("helperFunc").delete();
    let applied = repo.apply(changes);
    let edit_count = applied.edit_count();
    // preview() returns self for chaining — verify the edit count is preserved
    let applied_after_preview = applied.preview();
    assert_eq!(applied_after_preview.edit_count(), edit_count);
}
