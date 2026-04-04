use std::collections::HashMap;

use go_model::{ImportAlias, SourceFile};

/// Map from alias name to import path for a single file.
///
/// Dot imports are stored with the key `"."`.
/// Blank imports (`_`) are excluded — they exist only for side effects.
pub type AliasMap = HashMap<String, String>;

/// Build the alias map for a source file.
///
/// For each import in the file, computes the effective local alias and maps it
/// to the unquoted import path. Blank imports are skipped since they introduce
/// no names into scope.
pub fn build_alias_map(sf: &SourceFile) -> AliasMap {
    let mut map = AliasMap::new();

    for imp in &sf.imports {
        let path = imp.path.value();

        match &imp.alias {
            ImportAlias::Blank => continue,
            ImportAlias::Dot => {
                // Dot imports merge all exported names into the file scope.
                // We record the path under "." so callers can handle it.
                map.insert(".".to_owned(), path);
            }
            ImportAlias::Named(ident) => {
                map.insert(ident.name.clone(), path);
            }
            ImportAlias::Implicit => {
                // Last path component becomes the alias.
                // e.g. "encoding/json" → "json", "fmt" → "fmt"
                let alias = path.rsplit('/').next().unwrap_or(&path).to_owned();
                map.insert(alias, path);
            }
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use go_model::{Ident, ImportAlias, ImportSpec, Span, StringLit};

    use super::*;

    fn make_source_file(imports: Vec<ImportSpec>) -> SourceFile {
        SourceFile {
            package: Ident::synthetic("main"),
            imports,
            decls: vec![],
            span: Span::synthetic(),
        }
    }

    fn make_import(alias: ImportAlias, path: &str) -> ImportSpec {
        ImportSpec {
            alias,
            path: StringLit::from_value(path),
            span: Span::synthetic(),
        }
    }

    #[test]
    fn implicit_single_component() {
        let sf = make_source_file(vec![make_import(ImportAlias::Implicit, "fmt")]);
        let map = build_alias_map(&sf);
        assert_eq!(map.get("fmt").unwrap(), "fmt");
    }

    #[test]
    fn implicit_multi_component() {
        let sf = make_source_file(vec![make_import(ImportAlias::Implicit, "encoding/json")]);
        let map = build_alias_map(&sf);
        assert_eq!(map.get("json").unwrap(), "encoding/json");
    }

    #[test]
    fn named_alias() {
        let sf = make_source_file(vec![make_import(
            ImportAlias::Named(Ident::synthetic("j")),
            "encoding/json",
        )]);
        let map = build_alias_map(&sf);
        assert_eq!(map.get("j").unwrap(), "encoding/json");
        assert!(!map.contains_key("json"));
    }

    #[test]
    fn dot_import() {
        let sf = make_source_file(vec![make_import(ImportAlias::Dot, "testing")]);
        let map = build_alias_map(&sf);
        assert_eq!(map.get(".").unwrap(), "testing");
    }

    #[test]
    fn blank_import_excluded() {
        let sf = make_source_file(vec![make_import(ImportAlias::Blank, "net/http/pprof")]);
        let map = build_alias_map(&sf);
        assert!(map.is_empty());
    }

    #[test]
    fn mixed_imports() {
        let sf = make_source_file(vec![
            make_import(ImportAlias::Implicit, "fmt"),
            make_import(ImportAlias::Implicit, "encoding/json"),
            make_import(
                ImportAlias::Named(Ident::synthetic("pb")),
                "google/protobuf",
            ),
            make_import(ImportAlias::Dot, "math"),
            make_import(ImportAlias::Blank, "net/http/pprof"),
        ]);
        let map = build_alias_map(&sf);

        assert_eq!(map.len(), 4);
        assert_eq!(map["fmt"], "fmt");
        assert_eq!(map["json"], "encoding/json");
        assert_eq!(map["pb"], "google/protobuf");
        assert_eq!(map["."], "math");
    }

    #[test]
    fn empty_imports() {
        let sf = make_source_file(vec![]);
        let map = build_alias_map(&sf);
        assert!(map.is_empty());
    }
}
