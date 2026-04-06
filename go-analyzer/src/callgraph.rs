//! Call graph analysis: symbol table, call edge extraction, and reachability.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use go_model::*;

use crate::repo::Repo;
use crate::resolver::build_alias_map;

/// A fully-qualified symbol: package directory + name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol {
    /// Directory containing the package (used as package identity).
    pub pkg_dir: PathBuf,
    /// Symbol name (function, method, type, var, const).
    pub name: String,
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.pkg_dir.display(), self.name)
    }
}

/// The kind of a symbol in the symbol table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Func,
    Method { receiver_type: String },
    Type,
    Var,
    Const,
}

/// Entry in the symbol table.
#[derive(Debug, Clone)]
pub struct SymbolEntry {
    pub symbol: Symbol,
    pub kind: SymbolKind,
    pub span: Span,
    pub file: PathBuf,
    pub exported: bool,
}

/// A directed edge in the call graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallEdge {
    pub caller: Symbol,
    pub callee: Symbol,
}

/// Complete call graph for a repository.
pub struct CallGraph {
    pub symbols: HashMap<Symbol, SymbolEntry>,
    pub edges: Vec<CallEdge>,
    /// Adjacency list: caller → set of callees.
    pub calls: HashMap<Symbol, HashSet<Symbol>>,
    /// Reverse adjacency: callee → set of callers.
    pub called_by: HashMap<Symbol, HashSet<Symbol>>,
}

impl CallGraph {
    /// Build a call graph from a loaded Repo.
    pub fn build(repo: &Repo) -> Self {
        let mut symbols = HashMap::new();
        let mut edges = Vec::new();

        // Phase 1: Build symbol table
        for (path, rf) in &repo.files {
            let pkg_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            let alias_map = build_alias_map(&rf.ast);

            register_symbols(&rf.ast, &pkg_dir, path, &mut symbols);

            // Phase 2: Extract edges from declarations (signatures, types, bodies)
            for decl in &rf.ast.decls {
                // Extract type references from function/method signatures
                match decl {
                    TopLevelDecl::Func(f) => {
                        let func_sym = Symbol {
                            pkg_dir: pkg_dir.clone(),
                            name: f.name.name.clone(),
                        };
                        extract_refs_from_func_type(
                            &f.ty, &func_sym, &pkg_dir, &alias_map, repo, &mut edges,
                        );
                    }
                    TopLevelDecl::Method(m) => {
                        let recv_type = receiver_base_name(&m.receiver).unwrap_or_default();
                        let method_sym = Symbol {
                            pkg_dir: pkg_dir.clone(),
                            name: format!("{}.{}", recv_type, m.name.name),
                        };
                        extract_refs_from_type_expr(
                            &m.receiver.ty,
                            &method_sym,
                            &pkg_dir,
                            &alias_map,
                            repo,
                            &mut edges,
                        );
                        extract_refs_from_func_type(
                            &m.ty,
                            &method_sym,
                            &pkg_dir,
                            &alias_map,
                            repo,
                            &mut edges,
                        );
                    }
                    TopLevelDecl::Type(specs) => {
                        for spec in specs {
                            let type_sym = Symbol {
                                pkg_dir: pkg_dir.clone(),
                                name: spec.name().name.clone(),
                            };
                            extract_refs_from_type_expr(
                                spec.ty(),
                                &type_sym,
                                &pkg_dir,
                                &alias_map,
                                repo,
                                &mut edges,
                            );
                        }
                    }
                    _ => {}
                }
            }

            // Phase 3: Extract call edges from function/method bodies
            for decl in &rf.ast.decls {
                match decl {
                    TopLevelDecl::Func(f) => {
                        let caller = Symbol {
                            pkg_dir: pkg_dir.clone(),
                            name: f.name.name.clone(),
                        };
                        if let Some(body) = &f.body {
                            extract_calls_from_block(
                                body, &caller, &pkg_dir, &alias_map, repo, &mut edges,
                            );
                        }
                    }
                    TopLevelDecl::Method(m) => {
                        let recv_type = receiver_base_name(&m.receiver).unwrap_or_default();
                        let caller = Symbol {
                            pkg_dir: pkg_dir.clone(),
                            name: format!("{}.{}", recv_type, m.name.name),
                        };
                        if let Some(body) = &m.body {
                            extract_calls_from_block(
                                body, &caller, &pkg_dir, &alias_map, repo, &mut edges,
                            );
                        }
                    }
                    // Var/const init expressions: walk values and create edges
                    // FROM each var/const name TO the symbols it references.
                    TopLevelDecl::Var(specs) => {
                        for spec in specs {
                            for name in &spec.names {
                                let caller = Symbol {
                                    pkg_dir: pkg_dir.clone(),
                                    name: name.name.clone(),
                                };
                                for val in &spec.values {
                                    extract_calls_from_expr(
                                        val, &caller, &pkg_dir, &alias_map, repo, &mut edges,
                                    );
                                }
                                // Also reference the type if present
                                if let Some(ty) = &spec.ty {
                                    extract_refs_from_type_expr(
                                        ty, &caller, &pkg_dir, &alias_map, repo, &mut edges,
                                    );
                                }
                            }
                        }
                    }
                    TopLevelDecl::Const(specs) => {
                        for spec in specs {
                            for name in &spec.names {
                                let caller = Symbol {
                                    pkg_dir: pkg_dir.clone(),
                                    name: name.name.clone(),
                                };
                                for val in &spec.values {
                                    extract_calls_from_expr(
                                        val, &caller, &pkg_dir, &alias_map, repo, &mut edges,
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Post-process: expand wildcard method references (*.Name) to all
        // matching methods in the same package. This is the conservative
        // approach for method calls where we lack type inference.
        let wildcard_edges: Vec<_> = edges
            .iter()
            .filter(|e| e.callee.name.starts_with("*."))
            .cloned()
            .collect();
        for edge in &wildcard_edges {
            let method_name = &edge.callee.name[2..]; // strip "*."
            let pkg_dir = &edge.callee.pkg_dir;
            for (sym, _) in &symbols {
                if &sym.pkg_dir == pkg_dir && sym.name.ends_with(&format!(".{method_name}")) {
                    edges.push(CallEdge {
                        caller: edge.caller.clone(),
                        callee: sym.clone(),
                    });
                }
                // Also match plain function names (non-method)
                if &sym.pkg_dir == pkg_dir && sym.name == method_name {
                    edges.push(CallEdge {
                        caller: edge.caller.clone(),
                        callee: sym.clone(),
                    });
                }
            }
        }

        // Build adjacency lists
        let mut calls: HashMap<Symbol, HashSet<Symbol>> = HashMap::new();
        let mut called_by: HashMap<Symbol, HashSet<Symbol>> = HashMap::new();
        for edge in &edges {
            calls
                .entry(edge.caller.clone())
                .or_default()
                .insert(edge.callee.clone());
            called_by
                .entry(edge.callee.clone())
                .or_default()
                .insert(edge.caller.clone());
        }

        Self {
            symbols,
            edges,
            calls,
            called_by,
        }
    }

    /// Compute all symbols reachable from the given entry points via BFS.
    pub fn reachable_from(&self, entries: &[Symbol]) -> HashSet<Symbol> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        for entry in entries {
            if visited.insert(entry.clone()) {
                queue.push_back(entry.clone());
            }
        }

        while let Some(sym) = queue.pop_front() {
            if let Some(callees) = self.calls.get(&sym) {
                for callee in callees {
                    if visited.insert(callee.clone()) {
                        queue.push_back(callee.clone());
                    }
                }
            }
            // Also mark the type as reachable if this is a method
            if let Some(entry) = self.symbols.get(&sym)
                && let SymbolKind::Method { receiver_type } = &entry.kind
            {
                let type_sym = Symbol {
                    pkg_dir: sym.pkg_dir.clone(),
                    name: receiver_type.clone(),
                };
                if visited.insert(type_sym.clone()) {
                    queue.push_back(type_sym);
                }
            }
        }

        visited
    }

    /// Return all symbols NOT reachable from the entry points.
    pub fn unreachable_from(&self, entries: &[Symbol]) -> Vec<&SymbolEntry> {
        let reachable = self.reachable_from(entries);
        self.symbols
            .values()
            .filter(|entry| !reachable.contains(&entry.symbol))
            .collect()
    }
}

/// Register all top-level symbols from a source file into the symbol table.
fn register_symbols(
    sf: &SourceFile,
    pkg_dir: &Path,
    file: &Path,
    symbols: &mut HashMap<Symbol, SymbolEntry>,
) {
    for decl in &sf.decls {
        match decl {
            TopLevelDecl::Func(f) => {
                let sym = Symbol {
                    pkg_dir: pkg_dir.to_path_buf(),
                    name: f.name.name.clone(),
                };
                symbols.insert(
                    sym.clone(),
                    SymbolEntry {
                        symbol: sym,
                        kind: SymbolKind::Func,
                        span: f.span,
                        file: file.to_path_buf(),
                        exported: f.name.is_exported(),
                    },
                );
            }
            TopLevelDecl::Method(m) => {
                let recv_type = receiver_base_name(&m.receiver).unwrap_or_default();
                let sym = Symbol {
                    pkg_dir: pkg_dir.to_path_buf(),
                    name: format!("{}.{}", recv_type, m.name.name),
                };
                symbols.insert(
                    sym.clone(),
                    SymbolEntry {
                        symbol: sym,
                        kind: SymbolKind::Method {
                            receiver_type: recv_type,
                        },
                        span: m.span,
                        file: file.to_path_buf(),
                        exported: m.name.is_exported(),
                    },
                );
            }
            TopLevelDecl::Type(specs) => {
                for spec in specs {
                    let sym = Symbol {
                        pkg_dir: pkg_dir.to_path_buf(),
                        name: spec.name().name.clone(),
                    };
                    symbols.insert(
                        sym.clone(),
                        SymbolEntry {
                            symbol: sym,
                            kind: SymbolKind::Type,
                            span: spec.span(),
                            file: file.to_path_buf(),
                            exported: spec.name().is_exported(),
                        },
                    );
                }
            }
            TopLevelDecl::Var(specs) => {
                for spec in specs {
                    for name in &spec.names {
                        let sym = Symbol {
                            pkg_dir: pkg_dir.to_path_buf(),
                            name: name.name.clone(),
                        };
                        symbols.insert(
                            sym.clone(),
                            SymbolEntry {
                                symbol: sym,
                                kind: SymbolKind::Var,
                                span: spec.span,
                                file: file.to_path_buf(),
                                exported: name.is_exported(),
                            },
                        );
                    }
                }
            }
            TopLevelDecl::Const(specs) => {
                for spec in specs {
                    for name in &spec.names {
                        let sym = Symbol {
                            pkg_dir: pkg_dir.to_path_buf(),
                            name: name.name.clone(),
                        };
                        symbols.insert(
                            sym.clone(),
                            SymbolEntry {
                                symbol: sym,
                                kind: SymbolKind::Const,
                                span: spec.span,
                                file: file.to_path_buf(),
                                exported: name.is_exported(),
                            },
                        );
                    }
                }
            }
        }
    }
}

/// Extract call edges from a block of statements.
fn extract_calls_from_block(
    block: &Block,
    caller: &Symbol,
    pkg_dir: &Path,
    alias_map: &crate::resolver::AliasMap,
    repo: &Repo,
    edges: &mut Vec<CallEdge>,
) {
    for stmt in &block.stmts {
        extract_calls_from_stmt(stmt, caller, pkg_dir, alias_map, repo, edges);
    }
}

fn extract_calls_from_stmt(
    stmt: &Stmt,
    caller: &Symbol,
    pkg_dir: &Path,
    alias_map: &crate::resolver::AliasMap,
    repo: &Repo,
    edges: &mut Vec<CallEdge>,
) {
    match stmt {
        Stmt::Expr(e, _)
        | Stmt::Go(e, _)
        | Stmt::Defer(e, _)
        | Stmt::Inc(e, _)
        | Stmt::Dec(e, _) => {
            extract_calls_from_expr(e, caller, pkg_dir, alias_map, repo, edges);
        }
        Stmt::Return { values, .. } => {
            for v in values {
                extract_calls_from_expr(v, caller, pkg_dir, alias_map, repo, edges);
            }
        }
        Stmt::Assign { lhs, rhs, .. } => {
            for e in lhs.iter().chain(rhs.iter()) {
                extract_calls_from_expr(e, caller, pkg_dir, alias_map, repo, edges);
            }
        }
        Stmt::ShortVarDecl { values, .. } => {
            for v in values {
                extract_calls_from_expr(v, caller, pkg_dir, alias_map, repo, edges);
            }
        }
        Stmt::Send { channel, value, .. } => {
            extract_calls_from_expr(channel, caller, pkg_dir, alias_map, repo, edges);
            extract_calls_from_expr(value, caller, pkg_dir, alias_map, repo, edges);
        }
        Stmt::If {
            init,
            cond,
            body,
            else_,
            ..
        } => {
            if let Some(init) = init {
                extract_calls_from_stmt(init, caller, pkg_dir, alias_map, repo, edges);
            }
            extract_calls_from_expr(cond, caller, pkg_dir, alias_map, repo, edges);
            extract_calls_from_block(body, caller, pkg_dir, alias_map, repo, edges);
            if let Some(else_) = else_ {
                extract_calls_from_stmt(else_, caller, pkg_dir, alias_map, repo, edges);
            }
        }
        Stmt::For {
            init,
            cond,
            post,
            body,
            ..
        } => {
            if let Some(init) = init {
                extract_calls_from_stmt(init, caller, pkg_dir, alias_map, repo, edges);
            }
            if let Some(cond) = cond {
                extract_calls_from_expr(cond, caller, pkg_dir, alias_map, repo, edges);
            }
            if let Some(post) = post {
                extract_calls_from_stmt(post, caller, pkg_dir, alias_map, repo, edges);
            }
            extract_calls_from_block(body, caller, pkg_dir, alias_map, repo, edges);
        }
        Stmt::ForRange { iterable, body, .. } => {
            extract_calls_from_expr(iterable, caller, pkg_dir, alias_map, repo, edges);
            extract_calls_from_block(body, caller, pkg_dir, alias_map, repo, edges);
        }
        Stmt::Switch {
            init, tag, cases, ..
        } => {
            if let Some(init) = init {
                extract_calls_from_stmt(init, caller, pkg_dir, alias_map, repo, edges);
            }
            if let Some(tag) = tag {
                extract_calls_from_expr(tag, caller, pkg_dir, alias_map, repo, edges);
            }
            for case in cases {
                for e in &case.exprs {
                    extract_calls_from_expr(e, caller, pkg_dir, alias_map, repo, edges);
                }
                for s in &case.body {
                    extract_calls_from_stmt(s, caller, pkg_dir, alias_map, repo, edges);
                }
            }
        }
        Stmt::TypeSwitch {
            init,
            assign,
            cases,
            ..
        } => {
            if let Some(init) = init {
                extract_calls_from_stmt(init, caller, pkg_dir, alias_map, repo, edges);
            }
            extract_calls_from_expr(&assign.expr, caller, pkg_dir, alias_map, repo, edges);
            for case in cases {
                for s in &case.body {
                    extract_calls_from_stmt(s, caller, pkg_dir, alias_map, repo, edges);
                }
            }
        }
        Stmt::Select { cases, .. } => {
            for case in cases {
                match case {
                    CommCase::Send { stmt, body, .. } => {
                        extract_calls_from_stmt(stmt, caller, pkg_dir, alias_map, repo, edges);
                        for s in body {
                            extract_calls_from_stmt(s, caller, pkg_dir, alias_map, repo, edges);
                        }
                    }
                    CommCase::Recv {
                        stmt,
                        recv_expr,
                        body,
                        ..
                    } => {
                        if let Some(stmt) = stmt {
                            extract_calls_from_stmt(stmt, caller, pkg_dir, alias_map, repo, edges);
                        }
                        if let Some(expr) = recv_expr {
                            extract_calls_from_expr(expr, caller, pkg_dir, alias_map, repo, edges);
                        }
                        for s in body {
                            extract_calls_from_stmt(s, caller, pkg_dir, alias_map, repo, edges);
                        }
                    }
                    CommCase::Default { body, .. } => {
                        for s in body {
                            extract_calls_from_stmt(s, caller, pkg_dir, alias_map, repo, edges);
                        }
                    }
                }
            }
        }
        Stmt::Block(b) => {
            extract_calls_from_block(b, caller, pkg_dir, alias_map, repo, edges);
        }
        Stmt::Labeled { body, .. } => {
            extract_calls_from_stmt(body, caller, pkg_dir, alias_map, repo, edges);
        }
        Stmt::VarDecl(specs, _) => {
            for spec in specs {
                for v in &spec.values {
                    extract_calls_from_expr(v, caller, pkg_dir, alias_map, repo, edges);
                }
            }
        }
        Stmt::ConstDecl(specs, _) => {
            for spec in specs {
                for v in &spec.values {
                    extract_calls_from_expr(v, caller, pkg_dir, alias_map, repo, edges);
                }
            }
        }
        // Statements that don't contain expressions/calls
        Stmt::Break(..)
        | Stmt::Continue(..)
        | Stmt::Goto(..)
        | Stmt::Fallthrough(..)
        | Stmt::Empty(..)
        | Stmt::TypeDecl(..) => {}
    }
}

/// Extract call edges from an expression.
fn extract_calls_from_expr(
    expr: &Expr,
    caller: &Symbol,
    pkg_dir: &Path,
    alias_map: &crate::resolver::AliasMap,
    repo: &Repo,
    edges: &mut Vec<CallEdge>,
) {
    match expr {
        // Direct call: func(args) — resolve `func` to a symbol
        Expr::Call { func, args, .. } => {
            if let Some(callee) = resolve_call_target(func, pkg_dir, alias_map, repo) {
                edges.push(CallEdge {
                    caller: caller.clone(),
                    callee,
                });
            }
            // Also walk the function expression and arguments
            extract_calls_from_expr(func, caller, pkg_dir, alias_map, repo, edges);
            for arg in args {
                extract_calls_from_expr(arg, caller, pkg_dir, alias_map, repo, edges);
            }
        }
        // Identifier reference — could be a function value, type, or variable
        Expr::Ident(id) => {
            // Reference to a symbol in the same package.
            // Strip generic args if present: "readJsonl[Image]" → "readJsonl"
            let name = id.name.split('[').next().unwrap_or(&id.name).to_owned();
            let callee = Symbol {
                pkg_dir: pkg_dir.to_path_buf(),
                name,
            };
            edges.push(CallEdge {
                caller: caller.clone(),
                callee,
            });
        }
        // Qualified reference: pkg.Name
        Expr::Qualified { package, name, .. } => {
            if let Some(callee) = resolve_qualified(package, name, pkg_dir, alias_map, repo) {
                edges.push(CallEdge {
                    caller: caller.clone(),
                    callee,
                });
            }
        }
        // Selector: x.Method or x.Field — method call, field access, or pkg.Symbol
        Expr::Selector { operand, field, .. } => {
            extract_calls_from_expr(operand, caller, pkg_dir, alias_map, repo, edges);
            // If operand is a package identifier, resolve as pkg.Symbol
            if let Expr::Ident(pkg_ident) = operand.as_ref()
                && let Some(callee) = resolve_qualified(pkg_ident, field, pkg_dir, alias_map, repo)
            {
                edges.push(CallEdge {
                    caller: caller.clone(),
                    callee,
                });
            }
            // Conservative: without type inference, x.Name could be a method
            // call on any type. Use a wildcard marker "*.Name" that gets
            // expanded to all matching methods in the same package during
            // the post-processing phase.
            edges.push(CallEdge {
                caller: caller.clone(),
                callee: Symbol {
                    pkg_dir: pkg_dir.to_path_buf(),
                    name: format!("*.{}", field.name),
                },
            });
        }
        // Composite literal — references the type
        Expr::Composite { ty, elems, .. } => {
            if let Some(callee) = resolve_type_reference(ty, pkg_dir, alias_map, repo) {
                edges.push(CallEdge {
                    caller: caller.clone(),
                    callee,
                });
            }
            for elem in elems {
                if let Some(key) = &elem.key {
                    extract_calls_from_expr(key, caller, pkg_dir, alias_map, repo, edges);
                }
                extract_calls_from_expr(&elem.value, caller, pkg_dir, alias_map, repo, edges);
            }
        }
        // Function literal — walk the body
        Expr::FuncLit { body, .. } => {
            extract_calls_from_block(body, caller, pkg_dir, alias_map, repo, edges);
        }
        // Binary/unary — recurse into operands
        Expr::Binary { left, right, .. } => {
            extract_calls_from_expr(left, caller, pkg_dir, alias_map, repo, edges);
            extract_calls_from_expr(right, caller, pkg_dir, alias_map, repo, edges);
        }
        Expr::Unary { operand, .. } => {
            extract_calls_from_expr(operand, caller, pkg_dir, alias_map, repo, edges);
        }
        Expr::Paren(inner, _) => {
            extract_calls_from_expr(inner, caller, pkg_dir, alias_map, repo, edges);
        }
        Expr::Index { operand, index, .. } => {
            extract_calls_from_expr(operand, caller, pkg_dir, alias_map, repo, edges);
            extract_calls_from_expr(index, caller, pkg_dir, alias_map, repo, edges);
        }
        Expr::Slice {
            operand,
            low,
            high,
            max,
            ..
        } => {
            extract_calls_from_expr(operand, caller, pkg_dir, alias_map, repo, edges);
            if let Some(e) = low {
                extract_calls_from_expr(e, caller, pkg_dir, alias_map, repo, edges);
            }
            if let Some(e) = high {
                extract_calls_from_expr(e, caller, pkg_dir, alias_map, repo, edges);
            }
            if let Some(e) = max {
                extract_calls_from_expr(e, caller, pkg_dir, alias_map, repo, edges);
            }
        }
        Expr::TypeAssert { operand, .. } => {
            extract_calls_from_expr(operand, caller, pkg_dir, alias_map, repo, edges);
        }
        // Literals — no calls
        Expr::Int(_)
        | Expr::Float(_)
        | Expr::Imaginary(_)
        | Expr::Rune(_)
        | Expr::String(_)
        | Expr::RawString(_)
        | Expr::True(_)
        | Expr::False(_)
        | Expr::Nil(_)
        | Expr::Iota(_) => {}
    }
}

/// Try to resolve a call target expression to a symbol.
fn resolve_call_target(
    func: &Expr,
    pkg_dir: &Path,
    alias_map: &crate::resolver::AliasMap,
    repo: &Repo,
) -> Option<Symbol> {
    match func {
        // Direct call: myFunc(...) or myFunc[T](...) (generic instantiation)
        Expr::Ident(id) => {
            // Strip generic type args if present: "readJsonl[Image]" → "readJsonl"
            let name = id.name.split('[').next().unwrap_or(&id.name).to_owned();
            Some(Symbol {
                pkg_dir: pkg_dir.to_path_buf(),
                name,
            })
        }
        // Qualified call: pkg.Func(...)
        Expr::Qualified { package, name, .. } => {
            resolve_qualified(package, name, pkg_dir, alias_map, repo)
        }
        // Selector call: x.Method(...) — could be pkg.Func or method call
        Expr::Selector { operand, field, .. } => {
            if let Expr::Ident(pkg_ident) = operand.as_ref() {
                resolve_qualified(pkg_ident, field, pkg_dir, alias_map, repo)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Resolve a qualified reference (pkg.Name) to a symbol using the alias map.
fn resolve_qualified(
    package: &Ident,
    name: &Ident,
    _pkg_dir: &Path,
    alias_map: &crate::resolver::AliasMap,
    repo: &Repo,
) -> Option<Symbol> {
    let import_path = alias_map.get(&package.name)?;

    // Find the directory in the repo that corresponds to this import path.
    // We match by checking if any file's package directory ends with the
    // import path's last component(s).
    let target_dir = repo.files.keys().find_map(|path| {
        let dir = path.parent()?;
        let dir_str = dir.to_string_lossy();
        // Match import path against directory path suffix
        if dir_str.ends_with(import_path) || dir_str.ends_with(import_path.rsplit('/').next()?) {
            Some(dir.to_path_buf())
        } else {
            None
        }
    })?;

    Some(Symbol {
        pkg_dir: target_dir,
        name: name.name.clone(),
    })
}

/// Resolve a type reference to a symbol (for composite literals etc.).
fn resolve_type_reference(
    ty: &TypeExpr,
    pkg_dir: &Path,
    alias_map: &crate::resolver::AliasMap,
    repo: &Repo,
) -> Option<Symbol> {
    match ty {
        TypeExpr::Named(id) => Some(Symbol {
            pkg_dir: pkg_dir.to_path_buf(),
            name: id.name.clone(),
        }),
        TypeExpr::Qualified { package, name } => {
            resolve_qualified(package, name, pkg_dir, alias_map, repo)
        }
        _ => None,
    }
}

/// Extract type references from a function signature.
fn extract_refs_from_func_type(
    ft: &FuncType,
    caller: &Symbol,
    pkg_dir: &Path,
    alias_map: &crate::resolver::AliasMap,
    repo: &Repo,
    edges: &mut Vec<CallEdge>,
) {
    for param in ft.params.iter().chain(ft.results.iter()) {
        extract_refs_from_type_expr(&param.ty, caller, pkg_dir, alias_map, repo, edges);
    }
    for tp in &ft.type_params {
        extract_refs_from_type_expr(&tp.constraint, caller, pkg_dir, alias_map, repo, edges);
    }
}

/// Extract type references from a type expression, creating edges to referenced types.
fn extract_refs_from_type_expr(
    ty: &TypeExpr,
    caller: &Symbol,
    pkg_dir: &Path,
    alias_map: &crate::resolver::AliasMap,
    repo: &Repo,
    edges: &mut Vec<CallEdge>,
) {
    match ty {
        TypeExpr::Named(id) => {
            edges.push(CallEdge {
                caller: caller.clone(),
                callee: Symbol {
                    pkg_dir: pkg_dir.to_path_buf(),
                    name: id.name.clone(),
                },
            });
        }
        TypeExpr::Qualified { package, name } => {
            if let Some(callee) = resolve_qualified(package, name, pkg_dir, alias_map, repo) {
                edges.push(CallEdge {
                    caller: caller.clone(),
                    callee,
                });
            }
        }
        TypeExpr::Pointer(inner) => {
            extract_refs_from_type_expr(inner, caller, pkg_dir, alias_map, repo, edges);
        }
        TypeExpr::Slice(elem) => {
            extract_refs_from_type_expr(elem, caller, pkg_dir, alias_map, repo, edges);
        }
        TypeExpr::Array { elem, .. } => {
            extract_refs_from_type_expr(elem, caller, pkg_dir, alias_map, repo, edges);
        }
        TypeExpr::Map { key, value } => {
            extract_refs_from_type_expr(key, caller, pkg_dir, alias_map, repo, edges);
            extract_refs_from_type_expr(value, caller, pkg_dir, alias_map, repo, edges);
        }
        TypeExpr::Channel { elem, .. } => {
            extract_refs_from_type_expr(elem, caller, pkg_dir, alias_map, repo, edges);
        }
        TypeExpr::Func(ft) => {
            extract_refs_from_func_type(ft, caller, pkg_dir, alias_map, repo, edges);
        }
        TypeExpr::Struct(st) => {
            for field in &st.fields {
                let field_ty = match field {
                    FieldDecl::Named { ty, .. } => ty,
                    FieldDecl::Embedded { ty, .. } => ty,
                };
                extract_refs_from_type_expr(field_ty, caller, pkg_dir, alias_map, repo, edges);
            }
        }
        TypeExpr::Interface(it) => {
            for elem in &it.elements {
                match elem {
                    InterfaceElem::Method { ty, .. } => {
                        extract_refs_from_func_type(ty, caller, pkg_dir, alias_map, repo, edges);
                    }
                    InterfaceElem::Embedded(ty) => {
                        extract_refs_from_type_expr(ty, caller, pkg_dir, alias_map, repo, edges);
                    }
                    InterfaceElem::TypeTerm(tt) => {
                        for term in &tt.terms {
                            extract_refs_from_type_expr(
                                &term.ty, caller, pkg_dir, alias_map, repo, edges,
                            );
                        }
                    }
                }
            }
        }
        TypeExpr::Generic { base, args } => {
            extract_refs_from_type_expr(base, caller, pkg_dir, alias_map, repo, edges);
            for arg in args {
                extract_refs_from_type_expr(arg, caller, pkg_dir, alias_map, repo, edges);
            }
        }
    }
}

/// Extract the base type name from a receiver.
fn receiver_base_name(receiver: &Receiver) -> Option<String> {
    match &receiver.ty {
        TypeExpr::Named(id) => Some(id.name.clone()),
        TypeExpr::Pointer(inner) => match inner.as_ref() {
            TypeExpr::Named(id) => Some(id.name.clone()),
            TypeExpr::Generic { base, .. } => match base.as_ref() {
                TypeExpr::Named(id) => Some(id.name.clone()),
                _ => None,
            },
            _ => None,
        },
        TypeExpr::Generic { base, .. } => match base.as_ref() {
            TypeExpr::Named(id) => Some(id.name.clone()),
            _ => None,
        },
        _ => None,
    }
}
