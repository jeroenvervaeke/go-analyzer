use std::path::PathBuf;

use go_model::{Block, FuncDecl, MethodDecl, Receiver, TopLevelDecl, TypeExpr, TypeSpec};

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

/// An eager, filterable collection of AST items from a `Repo`.
pub struct Selection<'repo, T> {
    pub(crate) repo: &'repo Repo,
    pub(crate) items: Vec<SelectionItem<T>>,
}

impl<'repo, T> Selection<'repo, T> {
    pub fn filter(mut self, predicate: impl Fn(&T) -> bool) -> Self {
        self.items.retain(|si| predicate(&si.item));
        self
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    pub fn collect(&self) -> &[SelectionItem<T>] {
        &self.items
    }

    pub fn for_each(&self, mut f: impl FnMut(&T)) {
        for si in &self.items {
            f(&si.item);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn first(&self) -> Option<&SelectionItem<T>> {
        self.items.first()
    }
}

impl<'repo> Selection<'repo, FuncDecl> {
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

    pub fn exported(self) -> Self {
        self.filter(|f| f.name.is_exported())
    }

    pub fn unexported(self) -> Self {
        self.filter(|f| !f.name.is_exported())
    }

    pub fn excluding_tests(mut self) -> Self {
        self.items.retain(|si| {
            !si.file
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("_test.go"))
        });
        self
    }

    pub fn named(self, name: &str) -> Self {
        self.filter(|f| f.name.name == name)
    }

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

    pub fn exported(self) -> Self {
        self.filter(|m| m.name.is_exported())
    }

    pub fn unexported(self) -> Self {
        self.filter(|m| !m.name.is_exported())
    }

    pub fn excluding_tests(mut self) -> Self {
        self.items.retain(|si| {
            !si.file
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("_test.go"))
        });
        self
    }

    pub fn named(self, name: &str) -> Self {
        self.filter(|m| m.name.name == name)
    }

    pub fn on_type(self, type_name: &str) -> Self {
        self.filter(|m| receiver_type_name(&m.receiver) == Some(type_name))
    }

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

    pub fn exported(self) -> Self {
        self.filter(|t| t.name().is_exported())
    }

    pub fn unexported(self) -> Self {
        self.filter(|t| !t.name().is_exported())
    }

    pub fn excluding_tests(mut self) -> Self {
        self.items.retain(|si| {
            !si.file
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("_test.go"))
        });
        self
    }

    pub fn named(self, name: &str) -> Self {
        self.filter(|t| t.name().name == name)
    }

    pub fn structs(self) -> Self {
        self.filter(|t| t.is_struct())
    }

    pub fn interfaces(self) -> Self {
        self.filter(|t| t.is_interface())
    }

    /// Look up a named method on each selected type and return `MethodEntry`
    /// items for fluent add-or-modify patterns.
    pub fn method(self, method_name: &str) -> Selection<'repo, MethodEntry> {
        let mut entries = Vec::new();

        for si in &self.items {
            // Find matching methods in the repo for this type.
            let type_name = si.item.name().name.clone();
            let existing: Option<MethodDecl> = self
                .repo
                .files
                .iter()
                .flat_map(|(_, rf)| rf.ast.decls.iter())
                .filter_map(|d| match d {
                    TopLevelDecl::Method(m) => Some(m.as_ref()),
                    _ => None,
                })
                .find(|m| {
                    m.name.name == method_name
                        && receiver_type_name(&m.receiver) == Some(&type_name)
                })
                .cloned();

            entries.push(SelectionItem {
                item: MethodEntry {
                    type_spec: si.item.clone(),
                    method_name: method_name.to_owned(),
                    existing,
                    file: si.file.clone(),
                },
                file: si.file.clone(),
            });
        }

        Selection {
            repo: self.repo,
            items: entries,
        }
    }

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
                    if names.iter().any(|n| n.name == field_name) {
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

/// `MethodEntry` represents a method that may or may not exist on a type.
/// Used for fluent add-or-modify patterns.
#[derive(Debug, Clone)]
pub struct MethodEntry {
    pub type_spec: TypeSpec,
    pub method_name: String,
    pub existing: Option<MethodDecl>,
    pub file: PathBuf,
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
                    file: si.file.clone(),
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
                if existing.span.is_synthetic() {
                    return None;
                }
                let mut modified = existing.clone();
                modify(&mut modified);
                let new_text = Printer::method_decl(&modified);
                Some(Edit {
                    file: si.file.clone(),
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
                if existing.span.is_synthetic() {
                    return None;
                }
                Some(Edit {
                    file: si.file.clone(),
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
