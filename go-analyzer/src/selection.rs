use std::collections::HashMap;
use std::path::PathBuf;

use go_model::{
    Block, FuncDecl, FuncType, InterfaceElem, MethodDecl, Receiver, TopLevelDecl, TypeExpr,
    TypeSpec,
};

use crate::changes::Changes;
use crate::edit::{Edit, EditKind};
use crate::printer::Printer;
use crate::repo::Repo;

/// A located item wraps an AST node with its source file path.
#[derive(Debug, Clone)]
pub struct SelectionItem<T> {
    pub item: T,
    pub file: PathBuf,
}

/// An eager, filterable collection of AST items from a [`Repo`].
///
/// Built via [`Repo::functions`](crate::Repo::functions),
/// [`Repo::methods`](crate::Repo::methods), [`Repo::types`](crate::Repo::types), etc.
/// Chain filters to narrow the selection, then query or produce [`Changes`].
///
/// # Example
///
/// ```no_run
/// # use go_analyzer::*;
/// # use go_model::*;
/// let repo = Repo::load(".")?;
/// // Count unexported functions outside test files
/// let n = repo.functions().unexported().excluding_tests().count();
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct Selection<'repo, T> {
    pub(crate) repo: &'repo Repo,
    pub(crate) items: Vec<SelectionItem<T>>,
}

impl<'repo, T> Selection<'repo, T> {
    /// Retain only items that satisfy `predicate`.
    pub fn filter(mut self, predicate: impl Fn(&T) -> bool) -> Self {
        self.items.retain(|si| predicate(&si.item));
        self
    }

    /// Return the number of items in the selection.
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Return a slice of all items in the selection.
    pub fn collect(&self) -> &[SelectionItem<T>] {
        &self.items
    }

    /// Invoke `f` on each item in the selection.
    pub fn for_each(&self, mut f: impl FnMut(&T)) {
        for si in &self.items {
            f(&si.item);
        }
    }

    /// Return `true` if the selection contains no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return the first item, or `None` if the selection is empty.
    pub fn first(&self) -> Option<&SelectionItem<T>> {
        self.items.first()
    }
}

impl<'repo> Selection<'repo, FuncDecl> {
    /// Keep only functions declared in files whose `package` clause matches `package`.
    pub fn in_package(self, package: &str) -> Self {
        let matching_files: Vec<PathBuf> = self
            .repo
            .files
            .iter()
            .filter(|(_, rf)| rf.ast.package.name == package)
            .map(|(p, _)| p.clone())
            .collect();
        self.filter_by_files(&matching_files)
    }

    /// Keep only exported (capitalized) functions.
    pub fn exported(self) -> Self {
        self.filter(|f| f.name.is_exported())
    }

    /// Keep only unexported (lowercase) functions.
    pub fn unexported(self) -> Self {
        self.filter(|f| !f.name.is_exported())
    }

    /// Exclude functions defined in `_test.go` files.
    pub fn excluding_tests(mut self) -> Self {
        self.items.retain(|si| {
            !si.file
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("_test.go"))
        });
        self
    }

    /// Keep only functions whose name exactly matches `name`.
    pub fn named(self, name: &str) -> Self {
        self.filter(|f| f.name.name == name)
    }

    /// Produce [`Changes`] that delete all selected functions from their source files.
    pub fn delete(self) -> Changes {
        let edits = self
            .items
            .iter()
            .filter(|si| !si.item.span.is_synthetic())
            .map(|si| Edit {
                file: si.file.clone(),
                kind: EditKind::Delete { span: si.item.span },
            })
            .collect();
        Changes { edits }
    }

    /// Produce [`Changes`] that rename all selected functions to `new_name`.
    pub fn rename(self, new_name: &str) -> Changes {
        let edits = self
            .items
            .iter()
            .filter(|si| !si.item.name.span.is_synthetic())
            .map(|si| Edit {
                file: si.file.clone(),
                kind: EditKind::Replace {
                    span: si.item.name.span,
                    new_text: new_name.to_owned(),
                },
            })
            .collect();
        Changes { edits }
    }

    /// Produce [`Changes`] that replace the body of all selected functions with `body`.
    pub fn replace_body(self, body: Block) -> Changes {
        let edits = self
            .items
            .iter()
            .filter_map(|si| {
                if si.item.span.is_synthetic() {
                    return None;
                }
                // Replace entire function with a new printed version.
                let mut f = si.item.clone();
                f.body = Some(body.clone());
                let new_text = Printer::func_decl(&f);
                Some(Edit {
                    file: si.file.clone(),
                    kind: EditKind::Replace {
                        span: si.item.span,
                        new_text,
                    },
                })
            })
            .collect();
        Changes { edits }
    }
}

impl<'repo> Selection<'repo, MethodDecl> {
    /// Keep only methods declared in files whose `package` clause matches `package`.
    pub fn in_package(self, package: &str) -> Self {
        let matching_files: Vec<PathBuf> = self
            .repo
            .files
            .iter()
            .filter(|(_, rf)| rf.ast.package.name == package)
            .map(|(p, _)| p.clone())
            .collect();
        self.filter_by_files(&matching_files)
    }

    /// Keep only exported (capitalized) methods.
    pub fn exported(self) -> Self {
        self.filter(|m| m.name.is_exported())
    }

    /// Keep only unexported (lowercase) methods.
    pub fn unexported(self) -> Self {
        self.filter(|m| !m.name.is_exported())
    }

    /// Exclude methods defined in `_test.go` files.
    pub fn excluding_tests(mut self) -> Self {
        self.items.retain(|si| {
            !si.file
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("_test.go"))
        });
        self
    }

    /// Keep only methods whose name exactly matches `name`.
    pub fn named(self, name: &str) -> Self {
        self.filter(|m| m.name.name == name)
    }

    /// Keep only methods whose receiver type (ignoring pointer indirection) matches `type_name`.
    pub fn on_type(self, type_name: &str) -> Self {
        self.filter(|m| receiver_type_name(&m.receiver) == Some(type_name))
    }

    /// Produce [`Changes`] that delete all selected methods from their source files.
    pub fn delete(self) -> Changes {
        let edits = self
            .items
            .iter()
            .filter(|si| !si.item.span.is_synthetic())
            .map(|si| Edit {
                file: si.file.clone(),
                kind: EditKind::Delete { span: si.item.span },
            })
            .collect();
        Changes { edits }
    }

    /// Produce [`Changes`] that rename all selected methods to `new_name`.
    pub fn rename(self, new_name: &str) -> Changes {
        let edits = self
            .items
            .iter()
            .filter(|si| !si.item.name.span.is_synthetic())
            .map(|si| Edit {
                file: si.file.clone(),
                kind: EditKind::Replace {
                    span: si.item.name.span,
                    new_text: new_name.to_owned(),
                },
            })
            .collect();
        Changes { edits }
    }

    /// Produce [`Changes`] that replace the body of all selected methods with `body`.
    pub fn replace_body(self, body: Block) -> Changes {
        let edits = self
            .items
            .iter()
            .filter_map(|si| {
                if si.item.span.is_synthetic() {
                    return None;
                }
                let mut m = si.item.clone();
                m.body = Some(body.clone());
                let new_text = Printer::method_decl(&m);
                Some(Edit {
                    file: si.file.clone(),
                    kind: EditKind::Replace {
                        span: si.item.span,
                        new_text,
                    },
                })
            })
            .collect();
        Changes { edits }
    }
}

impl<'repo> Selection<'repo, TypeSpec> {
    /// Keep only types declared in files whose `package` clause matches `package`.
    pub fn in_package(self, package: &str) -> Self {
        let matching_files: Vec<PathBuf> = self
            .repo
            .files
            .iter()
            .filter(|(_, rf)| rf.ast.package.name == package)
            .map(|(p, _)| p.clone())
            .collect();
        self.filter_by_files(&matching_files)
    }

    /// Keep only exported (capitalized) types.
    pub fn exported(self) -> Self {
        self.filter(|t| t.name().is_exported())
    }

    /// Keep only unexported (lowercase) types.
    pub fn unexported(self) -> Self {
        self.filter(|t| !t.name().is_exported())
    }

    /// Exclude types defined in `_test.go` files.
    pub fn excluding_tests(mut self) -> Self {
        self.items.retain(|si| {
            !si.file
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("_test.go"))
        });
        self
    }

    /// Keep only types whose name exactly matches `name`.
    pub fn named(self, name: &str) -> Self {
        self.filter(|t| t.name().name == name)
    }

    /// Keep only struct types, filtering out interfaces, aliases, etc.
    pub fn structs(self) -> Self {
        self.filter(|t| t.is_struct())
    }

    /// Keep only interface types, filtering out structs, aliases, etc.
    pub fn interfaces(self) -> Self {
        self.filter(|t| t.is_interface())
    }

    /// Keep only types that implement the named interface.
    ///
    /// Checks whether each type's method set satisfies the interface's required
    /// methods. Method signatures are compared structurally (name, parameter
    /// types, and return types must match exactly).
    ///
    /// The interface is looked up by name in the repository. If the interface
    /// is not found, an empty selection is returned.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use go_analyzer::*;
    /// let repo = Repo::load(".").unwrap();
    /// let stringers = repo.structs().implementing("Stringer");
    /// println!("{} types implement Stringer", stringers.count());
    /// ```
    pub fn implementing(self, interface_name: &str) -> Self {
        // Find the interface definition in the repo
        let required_methods = match find_interface_methods(self.repo, interface_name) {
            Some(methods) => methods,
            None => {
                return Self {
                    repo: self.repo,
                    items: vec![],
                };
            }
        };

        // Collect all methods in the repo indexed by receiver type name
        let all_methods = collect_all_methods(self.repo);

        self.filter(|type_spec| {
            let type_name = &type_spec.name().name;
            let type_methods = all_methods.get(type_name.as_str());
            // Check: every required method has a matching implementation
            required_methods.iter().all(|req| {
                type_methods.is_some_and(|methods| {
                    methods
                        .iter()
                        .any(|(name, sig)| *name == req.0 && sig.signature_matches(&req.1))
                })
            })
        })
    }

    /// Look up a named method on each selected type and return `MethodEntry`
    /// items for fluent add-or-modify patterns.
    pub fn method(self, method_name: &str) -> Selection<'repo, MethodEntry> {
        let mut entries = Vec::new();

        for si in &self.items {
            // Find matching methods in the same package (directory) as the type.
            let type_name = si.item.name().name.clone();
            let type_dir = si.file.parent();
            let found: Option<(MethodDecl, PathBuf)> = self
                .repo
                .files
                .iter()
                .filter(|(path, _)| path.parent() == type_dir)
                .flat_map(|(path, rf)| {
                    rf.ast.decls.iter().filter_map(move |d| match d {
                        TopLevelDecl::Method(m) => Some((m.as_ref(), path)),
                        _ => None,
                    })
                })
                .find(|(m, _)| {
                    m.name.name == method_name
                        && receiver_type_name(&m.receiver) == Some(&type_name)
                })
                .map(|(m, path)| (m.clone(), path.clone()));

            let (existing, method_file) = match found {
                Some((m, path)) => (Some(m), Some(path)),
                None => (None, None),
            };

            entries.push(SelectionItem {
                item: MethodEntry {
                    type_spec: si.item.clone(),
                    method_name: method_name.to_owned(),
                    existing,
                    type_file: si.file.clone(),
                    method_file,
                },
                file: si.file.clone(),
            });
        }

        Selection {
            repo: self.repo,
            items: entries,
        }
    }

    /// Produce [`Changes`] that delete all selected types from their source files.
    pub fn delete(self) -> Changes {
        let edits = self
            .items
            .iter()
            .filter(|si| !si.item.span().is_synthetic())
            .map(|si| Edit {
                file: si.file.clone(),
                kind: EditKind::Delete {
                    span: si.item.span(),
                },
            })
            .collect();
        Changes { edits }
    }

    /// Produce [`Changes`] that rename all selected types to `new_name`.
    pub fn rename(self, new_name: &str) -> Changes {
        let edits = self
            .items
            .iter()
            .filter(|si| !si.item.name().span.is_synthetic())
            .map(|si| Edit {
                file: si.file.clone(),
                kind: EditKind::Replace {
                    span: si.item.name().span,
                    new_text: new_name.to_owned(),
                },
            })
            .collect();
        Changes { edits }
    }

    /// Add a named field to each selected struct type. Non-struct types are
    /// skipped. The field is inserted before the closing brace of the struct.
    pub fn add_field(self, field_name: &str, field_type: TypeExpr) -> Changes {
        let field_type_str = Printer::type_expr(&field_type);
        let edits = self
            .items
            .iter()
            .filter_map(|si| {
                let TypeSpec::Def {
                    ty: TypeExpr::Struct(st),
                    ..
                } = &si.item
                else {
                    return None;
                };
                if st.span.is_synthetic() {
                    return None;
                }
                // Insert before the closing brace of the struct body. The struct span's
                // end_byte points just after `}`. We insert a new field line just before.
                let insert_point = st.span.end_byte - 1;
                let new_text = format!("\t{field_name} {field_type_str}\n");
                Some(Edit {
                    file: si.file.clone(),
                    kind: EditKind::InsertAfter {
                        anchor_byte: insert_point,
                        new_text,
                    },
                })
            })
            .collect();
        Changes { edits }
    }

    /// Remove a named field from each selected struct type.
    pub fn remove_field(self, field_name: &str) -> Changes {
        let edits = self
            .items
            .iter()
            .filter_map(|si| {
                let TypeSpec::Def {
                    ty: TypeExpr::Struct(st),
                    ..
                } = &si.item
                else {
                    return None;
                };
                for field in &st.fields {
                    let go_model::FieldDecl::Named { names, span, .. } = field else {
                        continue;
                    };
                    if span.is_synthetic() {
                        continue;
                    }
                    if !names.iter().any(|n| n.name == field_name) {
                        continue;
                    }
                    // Only delete if this is the sole name in the field declaration.
                    // Multi-name fields like `X, Y int` can't be partially deleted
                    // via span replacement — skip them to avoid data loss.
                    if names.len() == 1 {
                        return Some(Edit {
                            file: si.file.clone(),
                            kind: EditKind::Delete { span: *span },
                        });
                    }
                }
                None
            })
            .collect();
        Changes { edits }
    }
}

/// A method that may or may not exist on a type.
///
/// Created by [`Selection<TypeSpec>::method`] for fluent add-or-modify patterns.
///
/// # Example
///
/// ```no_run
/// # use go_analyzer::*;
/// # use go_model::*;
/// let repo = Repo::load(".")?;
/// let changes = repo.structs().exported().method("String").or_add(|ts| {
///     build::method(
///         build::pointer_receiver("s", &ts.name().name),
///         "String",
///         vec![],
///         vec![build::unnamed_param(build::named("string"))],
///         build::block(vec![build::ret(vec![build::string("todo")])]),
///     )
/// });
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct MethodEntry {
    pub type_spec: TypeSpec,
    pub method_name: String,
    pub existing: Option<MethodDecl>,
    /// File containing the type declaration (used for or_add insertion).
    pub type_file: PathBuf,
    /// File containing the existing method (used for and_modify/delete edits).
    pub method_file: Option<PathBuf>,
}

impl<'repo> Selection<'repo, MethodEntry> {
    /// If the method does not exist, add it with the provided definition.
    /// Existing methods are left unchanged.
    pub fn or_add(self, make_method: impl Fn(&TypeSpec) -> MethodDecl) -> Changes {
        let edits = self
            .items
            .iter()
            .filter(|si| si.item.existing.is_none())
            .filter_map(|si| {
                let ts = &si.item.type_spec;
                if ts.span().is_synthetic() {
                    return None;
                }
                let method = make_method(ts);
                let printed = Printer::method_decl(&method);
                // Insert after the type declaration.
                Some(Edit {
                    file: si.item.type_file.clone(),
                    kind: EditKind::InsertAfter {
                        anchor_byte: ts.span().end_byte,
                        new_text: format!("\n\n{printed}"),
                    },
                })
            })
            .collect();
        Changes { edits }
    }

    /// Modify existing methods. Methods that don't exist are skipped.
    pub fn and_modify(self, modify: impl Fn(&mut MethodDecl)) -> Changes {
        let edits = self
            .items
            .iter()
            .filter_map(|si| {
                let existing = si.item.existing.as_ref()?;
                let method_file = si.item.method_file.as_ref()?;
                if existing.span.is_synthetic() {
                    return None;
                }
                let mut modified = existing.clone();
                modify(&mut modified);
                let new_text = Printer::method_decl(&modified);
                Some(Edit {
                    file: method_file.clone(),
                    kind: EditKind::Replace {
                        span: existing.span,
                        new_text,
                    },
                })
            })
            .collect();
        Changes { edits }
    }

    /// Delete existing methods. Missing methods are skipped.
    pub fn delete(self) -> Changes {
        let edits = self
            .items
            .iter()
            .filter_map(|si| {
                let existing = si.item.existing.as_ref()?;
                let method_file = si.item.method_file.as_ref()?;
                if existing.span.is_synthetic() {
                    return None;
                }
                Some(Edit {
                    file: method_file.clone(),
                    kind: EditKind::Delete {
                        span: existing.span,
                    },
                })
            })
            .collect();
        Changes { edits }
    }

    /// Filter to only entries where the method exists.
    pub fn existing(self) -> Self {
        Selection {
            repo: self.repo,
            items: self
                .items
                .into_iter()
                .filter(|si| si.item.existing.is_some())
                .collect(),
        }
    }

    /// Filter to only entries where the method is absent.
    pub fn absent(self) -> Self {
        Selection {
            repo: self.repo,
            items: self
                .items
                .into_iter()
                .filter(|si| si.item.existing.is_none())
                .collect(),
        }
    }
}

impl<'repo, T> Selection<'repo, T> {
    fn filter_by_files(mut self, files: &[PathBuf]) -> Self {
        self.items.retain(|si| files.contains(&si.file));
        self
    }
}

/// Find the required method signatures for a named interface in the repo.
/// Returns a list of (method_name, FuncType) pairs, or None if the interface
/// is not found.
fn find_interface_methods(repo: &Repo, interface_name: &str) -> Option<Vec<(String, FuncType)>> {
    for rf in repo.files.values() {
        for decl in &rf.ast.decls {
            let TopLevelDecl::Type(specs) = decl else {
                continue;
            };
            for spec in specs {
                if spec.name().name != interface_name {
                    continue;
                }
                let TypeExpr::Interface(iface) = spec.ty() else {
                    continue;
                };
                let methods: Vec<(String, FuncType)> = iface
                    .elements
                    .iter()
                    .filter_map(|elem| match elem {
                        InterfaceElem::Method { name, ty, .. } => {
                            Some((name.name.clone(), ty.clone()))
                        }
                        _ => None,
                    })
                    .collect();
                return Some(methods);
            }
        }
    }
    None
}

/// Collect all methods in the repo, grouped by receiver type name.
/// Returns a map from type name → list of (method_name, FuncType).
fn collect_all_methods(repo: &Repo) -> HashMap<&str, Vec<(&str, &FuncType)>> {
    let mut result: HashMap<&str, Vec<(&str, &FuncType)>> = HashMap::new();
    for rf in repo.files.values() {
        for decl in &rf.ast.decls {
            let TopLevelDecl::Method(m) = decl else {
                continue;
            };
            let Some(recv_name) = receiver_type_name(&m.receiver) else {
                continue;
            };
            result
                .entry(recv_name)
                .or_default()
                .push((&m.name.name, &m.ty));
        }
    }
    result
}

/// Extract the base type name from a receiver, stripping pointer indirection.
fn receiver_type_name(receiver: &Receiver) -> Option<&str> {
    match &receiver.ty {
        TypeExpr::Named(id) => Some(&id.name),
        TypeExpr::Pointer(inner) => match inner.as_ref() {
            TypeExpr::Named(id) => Some(&id.name),
            TypeExpr::Generic { base, .. } => match base.as_ref() {
                TypeExpr::Named(id) => Some(&id.name),
                _ => None,
            },
            _ => None,
        },
        TypeExpr::Generic { base, .. } => match base.as_ref() {
            TypeExpr::Named(id) => Some(&id.name),
            _ => None,
        },
        _ => None,
    }
}
