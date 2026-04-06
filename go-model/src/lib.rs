//! Pure data types representing the complete Go grammar as Rust types.
//!
//! `go-model` provides a 1:1 structural mapping of every Go language construct
//! as strongly-typed Rust enums and structs. All types derive `Serialize` and
//! `Deserialize` via serde.
//!
//! This crate contains no parsing logic, no tree-sitter dependency, and no I/O.
//!
//! # Key types
//!
//! - [`SourceFile`] -- top-level: package, imports, and top-level declarations
//! - [`TopLevelDecl`] -- functions, methods, type definitions, vars, consts
//! - [`TypeExpr`] -- every Go type expression (named, pointer, slice, map, channel, func, etc.)
//! - [`Expr`] -- every Go expression (identifiers, literals, calls, selectors, operators, etc.)
//! - [`Stmt`] -- every Go statement (if, for, switch, select, return, assign, etc.)
//! - [`Span`] -- source location; [`Span::synthetic()`] marks generated nodes
//!
//! # Code generation
//!
//! The [`build`] module provides ergonomic constructors for generating AST nodes:
//!
//! ```
//! use go_model::build;
//!
//! let ret_stmt = build::ret(vec![
//!     build::call(
//!         build::selector(build::ident("fmt"), "Sprintf"),
//!         vec![build::string("%+v"), build::deref(build::ident("x"))],
//!     ),
//! ]);
//! ```

pub mod build;
mod decl;
mod expr;
mod leaf;
mod span;
mod stmt;
mod types;

pub use decl::*;
pub use expr::*;
pub use leaf::*;
pub use span::Span;
pub use stmt::*;
pub use types::*;

#[cfg(test)]
mod tests;
