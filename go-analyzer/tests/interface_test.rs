use go_analyzer::Repo;

fn fixture_repo() -> Repo {
    let path = format!("{}/tests/fixture_repo", env!("CARGO_MANIFEST_DIR"));
    Repo::load(&path).expect("failed to load fixture repo")
}

#[test]
fn test_implementing_stringer() {
    let repo = fixture_repo();
    // User has String() string → implements Stringer
    // Server has String() string → implements Stringer
    // Admin, Config, Client do NOT have String() → don't implement Stringer
    let stringers = repo.structs().implementing("Stringer");
    let names: Vec<_> = stringers
        .collect()
        .into_iter()
        .map(|si| si.item.name().name.clone())
        .collect();

    assert!(
        names.contains(&"User".to_owned()),
        "User should implement Stringer: {names:?}"
    );
    assert!(
        names.contains(&"Server".to_owned()),
        "Server should implement Stringer: {names:?}"
    );
    assert!(
        !names.contains(&"Admin".to_owned()),
        "Admin should not implement Stringer: {names:?}"
    );
    assert!(
        !names.contains(&"Config".to_owned()),
        "Config should not implement Stringer: {names:?}"
    );
    assert!(
        !names.contains(&"Client".to_owned()),
        "Client should not implement Stringer: {names:?}"
    );
}

#[test]
fn test_implementing_starter() {
    let repo = fixture_repo();
    // Server has Start() error → implements Starter
    // Nothing else has Start()
    let starters = repo.structs().implementing("Starter");
    let names: Vec<_> = starters
        .collect()
        .into_iter()
        .map(|si| si.item.name().name.clone())
        .collect();

    assert_eq!(
        names.len(),
        1,
        "only Server should implement Starter: {names:?}"
    );
    assert!(names.contains(&"Server".to_owned()));
}

#[test]
fn test_implementing_count() {
    let repo = fixture_repo();
    assert_eq!(repo.structs().implementing("Stringer").count(), 2);
    assert_eq!(repo.structs().implementing("Starter").count(), 1);
}

#[test]
fn test_implementing_unknown_interface() {
    let repo = fixture_repo();
    // NonExistent interface → empty selection
    assert_eq!(repo.structs().implementing("NonExistent").count(), 0);
}

#[test]
fn test_implementing_chained_with_exported() {
    let repo = fixture_repo();
    // All Stringer implementors that are exported
    let count = repo.structs().exported().implementing("Stringer").count();
    // Both User and Server are exported
    assert_eq!(count, 2);
}

#[test]
fn test_implementing_empty_interface() {
    // Every type implements an empty interface
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("pkg")).unwrap();
    std::fs::write(
        tmp.path().join("pkg/types.go"),
        "package pkg\n\ntype Any interface{}\n\ntype Foo struct{}\n\ntype Bar struct{}\n",
    )
    .unwrap();

    let repo = Repo::load(tmp.path()).unwrap();
    // Empty interface has no required methods → every struct satisfies it
    let count = repo.structs().implementing("Any").count();
    assert_eq!(count, 2, "all structs implement empty interface");
}

#[test]
fn test_implementing_multi_method_interface() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("pkg")).unwrap();
    std::fs::write(
        tmp.path().join("pkg/types.go"),
        r#"package pkg

type ReadWriter interface {
	Read(p []byte) (int, error)
	Write(p []byte) (int, error)
}

type Full struct{}

func (f *Full) Read(p []byte) (int, error) { return 0, nil }
func (f *Full) Write(p []byte) (int, error) { return 0, nil }

type ReadOnly struct{}

func (r *ReadOnly) Read(p []byte) (int, error) { return 0, nil }

type Empty struct{}
"#,
    )
    .unwrap();

    let repo = Repo::load(tmp.path()).unwrap();

    let rw = repo.structs().implementing("ReadWriter");
    let names: Vec<_> = rw
        .collect()
        .into_iter()
        .map(|si| si.item.name().name.clone())
        .collect();

    assert!(
        names.contains(&"Full".to_owned()),
        "Full implements ReadWriter: {names:?}"
    );
    assert!(
        !names.contains(&"ReadOnly".to_owned()),
        "ReadOnly only has Read: {names:?}"
    );
    assert!(
        !names.contains(&"Empty".to_owned()),
        "Empty has nothing: {names:?}"
    );
}

#[test]
fn test_implementing_partial_three_method_interface() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("pkg")).unwrap();
    std::fs::write(
        tmp.path().join("pkg/types.go"),
        r#"package pkg

type Storage interface {
	Get(key string) (string, error)
	Set(key string, value string) error
	Delete(key string) error
}

type FullStore struct{}

func (s *FullStore) Get(key string) (string, error) { return "", nil }
func (s *FullStore) Set(key string, value string) error { return nil }
func (s *FullStore) Delete(key string) error { return nil }

type PartialStore struct{}

func (s *PartialStore) Get(key string) (string, error) { return "", nil }
func (s *PartialStore) Set(key string, value string) error { return nil }
"#,
    )
    .unwrap();

    let repo = Repo::load(tmp.path()).unwrap();

    let stores = repo.structs().implementing("Storage");
    let names: Vec<_> = stores
        .collect()
        .into_iter()
        .map(|si| si.item.name().name.clone())
        .collect();

    assert!(
        names.contains(&"FullStore".to_owned()),
        "FullStore has all 3 methods: {names:?}"
    );
    assert!(
        !names.contains(&"PartialStore".to_owned()),
        "PartialStore only has 2 of 3 methods: {names:?}"
    );
}
