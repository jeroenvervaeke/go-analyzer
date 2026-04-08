use go_analyzer::Repo;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::output::QueryItem;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SelectKind {
    Functions,
    Methods,
    Structs,
    Interfaces,
    Types,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    Named(String),
    InPackage(String),
    Exported(bool),
    ExcludingTests(bool),
    OnType(String),
    Implementing(String),
}

pub fn build_query(repo: &Repo, select: &SelectKind, filters: &[Filter]) -> Vec<QueryItem> {
    match select {
        SelectKind::Functions => query_functions(repo, filters),
        SelectKind::Methods => query_methods(repo, filters),
        SelectKind::Structs => query_type_specs(repo, filters, TypeFilter::Structs),
        SelectKind::Interfaces => query_type_specs(repo, filters, TypeFilter::Interfaces),
        SelectKind::Types => query_type_specs(repo, filters, TypeFilter::All),
    }
}

fn query_functions(repo: &Repo, filters: &[Filter]) -> Vec<QueryItem> {
    let mut sel = repo.functions();

    for f in filters {
        match f {
            Filter::Named(name) => sel = sel.named(name),
            Filter::InPackage(pkg) => sel = sel.in_package(pkg),
            Filter::Exported(true) => sel = sel.exported(),
            Filter::Exported(false) => sel = sel.unexported(),
            Filter::ExcludingTests(true) => sel = sel.excluding_tests(),
            // OnType and Implementing are not applicable to functions -- silently ignore
            Filter::ExcludingTests(false) | Filter::OnType(_) | Filter::Implementing(_) => {}
        }
    }

    sel.collect()
        .iter()
        .map(|si| {
            let qi = QueryItem::from_func(&si.item, &si.file);
            match repo.package_for_file(&si.file) {
                Some(pkg) => qi.with_package(pkg),
                None => qi,
            }
        })
        .collect()
}

fn query_methods(repo: &Repo, filters: &[Filter]) -> Vec<QueryItem> {
    let mut sel = repo.methods();

    for f in filters {
        match f {
            Filter::Named(name) => sel = sel.named(name),
            Filter::InPackage(pkg) => sel = sel.in_package(pkg),
            Filter::Exported(true) => sel = sel.exported(),
            Filter::Exported(false) => sel = sel.unexported(),
            Filter::ExcludingTests(true) => sel = sel.excluding_tests(),
            Filter::OnType(type_name) => sel = sel.on_type(type_name),
            // Implementing is not applicable to methods -- silently ignore
            Filter::ExcludingTests(false) | Filter::Implementing(_) => {}
        }
    }

    sel.collect()
        .iter()
        .map(|si| {
            let qi = QueryItem::from_method(&si.item, &si.file);
            match repo.package_for_file(&si.file) {
                Some(pkg) => qi.with_package(pkg),
                None => qi,
            }
        })
        .collect()
}

enum TypeFilter {
    Structs,
    Interfaces,
    All,
}

fn query_type_specs(repo: &Repo, filters: &[Filter], type_filter: TypeFilter) -> Vec<QueryItem> {
    let mut sel = match type_filter {
        TypeFilter::Structs => repo.structs(),
        TypeFilter::Interfaces => repo.interfaces(),
        TypeFilter::All => repo.types(),
    };

    for f in filters {
        match f {
            Filter::Named(name) => sel = sel.named(name),
            Filter::InPackage(pkg) => sel = sel.in_package(pkg),
            Filter::Exported(true) => sel = sel.exported(),
            Filter::Exported(false) => sel = sel.unexported(),
            Filter::ExcludingTests(true) => sel = sel.excluding_tests(),
            Filter::Implementing(iface) => sel = sel.implementing(iface),
            // OnType is not applicable to types -- silently ignore
            Filter::ExcludingTests(false) | Filter::OnType(_) => {}
        }
    }

    sel.collect()
        .iter()
        .map(|si| {
            let qi = QueryItem::from_type_spec(&si.item, &si.file);
            match repo.package_for_file(&si.file) {
                Some(pkg) => qi.with_package(pkg),
                None => qi,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_repo() -> Repo {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../go-analyzer/tests/fixture_repo");
        Repo::load(path).expect("failed to load fixture repo")
    }

    #[test]
    fn test_query_all_functions() {
        let repo = fixture_repo();
        let items = build_query(&repo, &SelectKind::Functions, &[]);
        // alpha: NewUser, NewConfig, helperFunc; beta: RunServer
        assert!(
            items.len() >= 3,
            "expected >= 3 functions, got {}",
            items.len()
        );
    }

    #[test]
    fn test_query_exported_functions() {
        let repo = fixture_repo();
        let items = build_query(&repo, &SelectKind::Functions, &[Filter::Exported(true)]);
        assert!(!items.is_empty());
        for item in &items {
            assert!(item.exported, "{} should be exported", item.name);
        }
    }

    #[test]
    fn test_query_methods_on_type() {
        let repo = fixture_repo();
        let items = build_query(
            &repo,
            &SelectKind::Methods,
            &[Filter::OnType("Server".to_owned())],
        );
        assert!(!items.is_empty());
        for item in &items {
            let recv = item.receiver.as_ref().expect("method should have receiver");
            assert!(
                recv.contains("Server"),
                "receiver should contain Server, got: {recv}"
            );
        }
    }

    #[test]
    fn test_query_structs_implementing_interface() {
        let repo = fixture_repo();
        let items = build_query(
            &repo,
            &SelectKind::Structs,
            &[Filter::Implementing("Stringer".to_owned())],
        );
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert!(
            names.contains(&"User"),
            "User should implement Stringer, got: {names:?}"
        );
        assert!(
            names.contains(&"Server"),
            "Server should implement Stringer, got: {names:?}"
        );
    }

    #[test]
    fn test_query_named_function() {
        let repo = fixture_repo();
        let items = build_query(
            &repo,
            &SelectKind::Functions,
            &[Filter::Named("NewUser".to_owned())],
        );
        assert_eq!(
            items.len(),
            1,
            "expected exactly 1 result, got {:?}",
            items.iter().map(|i| &i.name).collect::<Vec<_>>()
        );
        assert_eq!(items[0].name, "NewUser");
    }

    #[test]
    fn test_query_in_package() {
        let repo = fixture_repo();
        let items = build_query(
            &repo,
            &SelectKind::Functions,
            &[Filter::InPackage("beta".to_owned())],
        );
        assert!(!items.is_empty());
        for item in &items {
            assert_eq!(
                item.package, "beta",
                "expected package beta, got: {}",
                item.package
            );
        }
    }

    #[test]
    fn test_inapplicable_filter_is_ignored() {
        let repo = fixture_repo();
        let unfiltered = build_query(&repo, &SelectKind::Functions, &[]);
        let with_on_type = build_query(
            &repo,
            &SelectKind::Functions,
            &[Filter::OnType("Server".to_owned())],
        );
        assert_eq!(
            unfiltered.len(),
            with_on_type.len(),
            "OnType filter should be ignored for functions"
        );
    }
}
