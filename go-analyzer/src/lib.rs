//! Analyze and transform Go source code from Rust with a fluent, type-safe API.
//!
//! `go-analyzer` parses Go repositories into a complete typed AST
//! ([`go_model`]), then provides a pipeline for querying, rewriting, and
//! committing changes back to disk.
//!
//! # Workflow
//!
//! ```text
//! Repo::load(".")  ->  Selection<T>  ->  query / Changes  ->  Applied  ->  commit
//! ```
//!
//! 1. **Load** a repository with [`Repo::load`].
//! 2. **Select** items via the fluent API: [`Repo::structs`], [`Repo::functions`], etc.
//! 3. **Query** (`.count()`, `.collect()`, `.for_each()`) or **change**
//!    (`.delete()`, `.rename()`, `.or_add()`) -- both are pure, no side effects.
//! 4. **Apply** changes with [`Repo::apply`] to get an [`Applied`] value.
//! 5. **Preview** (`.preview()`) or **commit** (`.commit()`) to write to disk.
//!
//! # Example
//!
//! ```no_run
//! use go_analyzer::{Repo, build};
//!
//! let repo = Repo::load(".")?;
//!
//! // Add String() to exported structs that don't have one
//! let changes = repo.structs().exported().method("String").or_add(|ts| {
//!     let name = &ts.name().name;
//!     build::method(
//!         build::pointer_receiver("x", name),
//!         "String",
//!         vec![],
//!         vec![build::unnamed_param(build::named("string"))],
//!         build::block(vec![build::ret(vec![build::call(
//!             build::selector(build::ident("fmt"), "Sprintf"),
//!             vec![build::string("%+v"), build::deref(build::ident("x"))],
//!         )])]),
//!     )
//! });
//!
//! repo.apply(changes).preview().commit()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod applied;
pub mod callgraph;
pub mod changes;
pub mod edit;
pub mod printer;
pub mod repo;
pub mod resolver;
pub mod selection;
pub mod walker;

pub use go_model;
pub use go_model::build;

pub use applied::{Applied, CommitSummary};
pub use changes::Changes;
pub use repo::Repo;
pub use selection::{MethodEntry, Selection, SelectionItem};

/// Test-only helpers that expose internal printer for integration tests.
#[doc(hidden)]
pub mod test_support {
    use go_model::{FuncDecl, MethodDecl};

    pub fn print_func_decl(f: &FuncDecl) -> String {
        crate::printer::Printer::func_decl(f)
    }

    pub fn print_method_decl(m: &MethodDecl) -> String {
        crate::printer::Printer::method_decl(m)
    }
}
