use std::path::{Path, PathBuf};

use go_analyzer::printer::Printer;
use go_model::{
    ConstSpec, FuncDecl, MethodDecl, SourceFile, TopLevelDecl, TypeExpr, TypeSpec, VarSpec,
};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct QueryItem {
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>,
    pub package: String,
    pub file: PathBuf,
    pub line: usize,
    pub end_line: usize,
    pub exported: bool,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

impl QueryItem {
    pub fn from_func(f: &FuncDecl, file: &Path) -> Self {
        let sig = func_signature(&f.ty);
        Self {
            name: f.name.name.clone(),
            kind: "function".to_owned(),
            receiver: None,
            package: String::new(),
            file: file.to_owned(),
            line: f.span.start_row + 1,
            end_line: f.span.end_row + 1,
            exported: f.name.is_exported(),
            signature: format!("func {}{sig}", f.name.name),
            doc: f.doc.clone(),
        }
    }

    pub fn from_method(m: &MethodDecl, file: &Path) -> Self {
        let recv = format_receiver(&m.receiver);
        let sig = func_signature(&m.ty);
        Self {
            name: m.name.name.clone(),
            kind: "method".to_owned(),
            receiver: Some(recv.clone()),
            package: String::new(),
            file: file.to_owned(),
            line: m.span.start_row + 1,
            end_line: m.span.end_row + 1,
            exported: m.name.is_exported(),
            signature: format!("func ({recv}) {}{sig}", m.name.name),
            doc: m.doc.clone(),
        }
    }

    pub fn from_type_spec(t: &TypeSpec, file: &Path) -> Self {
        let kind = match t.ty() {
            TypeExpr::Struct(_) => "struct",
            TypeExpr::Interface(_) => "interface",
            _ => "type",
        };
        let type_str = Printer::type_expr(t.ty());
        Self {
            name: t.name().name.clone(),
            kind: kind.to_owned(),
            receiver: None,
            package: String::new(),
            file: file.to_owned(),
            line: t.span().start_row + 1,
            end_line: t.span().end_row + 1,
            exported: t.name().is_exported(),
            signature: format!("type {} {type_str}", t.name().name),
            doc: None,
        }
    }

    pub fn with_package(mut self, pkg: &str) -> Self {
        self.package = pkg.to_owned();
        self
    }
}

/// Render a FuncType as its signature portion: "(params) results".
/// Uses the Printer to render a func type, then strips the leading "func".
fn func_signature(ft: &go_model::FuncType) -> String {
    let full = Printer::type_expr(&TypeExpr::Func(ft.clone()));
    // Printer::type_expr for Func produces "func(params) results"
    full.strip_prefix("func").unwrap_or(&full).to_owned()
}

/// Format a receiver as "name type" (e.g. "u *User").
fn format_receiver(recv: &go_model::Receiver) -> String {
    let ty_str = Printer::type_expr(&recv.ty);
    match &recv.name {
        Some(name) => format!("{} {ty_str}", name.name),
        None => ty_str,
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FileOverview {
    pub package: String,
    pub file: PathBuf,
    pub imports: Vec<String>,
    pub types: Vec<TypeOverview>,
    pub functions: Vec<FunctionOverview>,
    pub methods: Vec<MethodOverview>,
    pub constants: Vec<ValueOverview>,
    pub variables: Vec<ValueOverview>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TypeOverview {
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub exported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FunctionOverview {
    pub name: String,
    pub line: usize,
    pub signature: String,
    pub exported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MethodOverview {
    pub name: String,
    pub receiver: String,
    pub line: usize,
    pub signature: String,
    pub exported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ValueOverview {
    pub name: String,
    pub line: usize,
    pub exported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ModuleOverview {
    pub module: String,
    pub path: PathBuf,
    pub packages: Vec<PackageOverview>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PackageOverview {
    pub name: String,
    pub import_path: String,
    pub path: PathBuf,
    pub files: Vec<String>,
    pub types: usize,
    pub functions: usize,
    pub methods: usize,
    pub constants: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CallGraphResult {
    pub nodes: Vec<CallGraphNode>,
    pub edges: Vec<CallGraphEdge>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CallGraphNode {
    pub symbol: String,
    pub file: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CallGraphEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EditResult {
    pub diff: String,
    pub files_modified: Vec<PathBuf>,
    pub edits_applied: usize,
}

pub fn build_file_overview(
    source_file: &SourceFile,
    file_path: &Path,
    include_docs: bool,
) -> FileOverview {
    let imports = source_file
        .imports
        .iter()
        .map(|imp| imp.path.value())
        .collect();

    let mut types = Vec::new();
    let mut functions = Vec::new();
    let mut methods = Vec::new();
    let mut constants = Vec::new();
    let mut variables = Vec::new();

    for decl in &source_file.decls {
        match decl {
            TopLevelDecl::Func(f) => {
                let sig = func_signature(&f.ty);
                functions.push(FunctionOverview {
                    name: f.name.name.clone(),
                    line: f.span.start_row + 1,
                    signature: format!("func {}{sig}", f.name.name),
                    exported: f.name.is_exported(),
                    doc: if include_docs { f.doc.clone() } else { None },
                });
            }
            TopLevelDecl::Method(m) => {
                let recv = format_receiver(&m.receiver);
                let sig = func_signature(&m.ty);
                methods.push(MethodOverview {
                    name: m.name.name.clone(),
                    receiver: recv.clone(),
                    line: m.span.start_row + 1,
                    signature: format!("func ({recv}) {}{sig}", m.name.name),
                    exported: m.name.is_exported(),
                    doc: if include_docs { m.doc.clone() } else { None },
                });
            }
            TopLevelDecl::Type(specs) => {
                for spec in specs {
                    let kind = match spec.ty() {
                        TypeExpr::Struct(_) => "struct",
                        TypeExpr::Interface(_) => "interface",
                        _ => "type",
                    };
                    types.push(TypeOverview {
                        name: spec.name().name.clone(),
                        kind: kind.to_owned(),
                        line: spec.span().start_row + 1,
                        exported: spec.name().is_exported(),
                        doc: None,
                    });
                }
            }
            TopLevelDecl::Const(specs) => {
                for spec in specs {
                    collect_values(spec, &mut constants);
                }
            }
            TopLevelDecl::Var(specs) => {
                for spec in specs {
                    collect_var_values(spec, &mut variables);
                }
            }
        }
    }

    FileOverview {
        package: source_file.package.name.clone(),
        file: file_path.to_owned(),
        imports,
        types,
        functions,
        methods,
        constants,
        variables,
    }
}

fn collect_values(spec: &ConstSpec, out: &mut Vec<ValueOverview>) {
    for (i, name) in spec.names.iter().enumerate() {
        let value = spec.values.get(i).map(Printer::expr);
        out.push(ValueOverview {
            name: name.name.clone(),
            line: spec.span.start_row + 1,
            exported: name.is_exported(),
            value,
        });
    }
}

fn collect_var_values(spec: &VarSpec, out: &mut Vec<ValueOverview>) {
    for (i, name) in spec.names.iter().enumerate() {
        let value = spec.values.get(i).map(Printer::expr);
        out.push(ValueOverview {
            name: name.name.clone(),
            line: spec.span.start_row + 1,
            exported: name.is_exported(),
            value,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use go_analyzer::Repo;

    fn fixture_repo() -> Repo {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../go-analyzer/tests/fixture_repo");
        Repo::load(path).expect("failed to load fixture repo")
    }

    #[test]
    fn test_query_item_from_func_has_location() {
        let repo = fixture_repo();
        let funcs = repo.functions();
        let item = funcs
            .collect()
            .iter()
            .find(|si| si.item.name.name == "NewUser")
            .expect("NewUser not found");

        let qi = QueryItem::from_func(&item.item, &item.file);
        assert_eq!(qi.name, "NewUser");
        assert_eq!(qi.kind, "function");
        assert!(qi.line > 0);
        assert!(qi.file.to_string_lossy().ends_with(".go"));
        assert!(qi.exported);
        assert!(qi.signature.contains("NewUser"));
    }

    #[test]
    fn test_query_item_from_method_has_receiver() {
        let repo = fixture_repo();
        let methods = repo.methods();
        let item = methods
            .collect()
            .iter()
            .find(|si| si.item.name.name == "String" && si.file.to_string_lossy().contains("alpha"))
            .expect("String method not found in alpha");

        let qi = QueryItem::from_method(&item.item, &item.file);
        assert!(qi.receiver.is_some());
        let recv = qi.receiver.as_ref().unwrap();
        assert!(
            recv.contains("User"),
            "receiver should contain User, got: {recv}"
        );
    }

    #[test]
    fn test_query_item_from_type_spec() {
        let repo = fixture_repo();
        let structs = repo.structs();
        let item = structs
            .collect()
            .iter()
            .find(|si| si.item.name().name == "User")
            .expect("User struct not found");

        let qi = QueryItem::from_type_spec(&item.item, &item.file);
        assert_eq!(qi.kind, "struct");
        assert!(qi.exported);
    }
}
