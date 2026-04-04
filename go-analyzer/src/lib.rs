pub mod applied;
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
