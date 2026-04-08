use std::sync::{Arc, Mutex};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use crate::output::{CallGraphResult, EditResult, FileOverview, ModuleOverview};
use crate::state::ServerState;
use crate::tools::call_graph::{CallGraphInput, handle_call_graph};
use crate::tools::describe::{
    DescribeFileInput, DescribeModuleInput, handle_describe_file, handle_describe_module,
};
use crate::tools::edit::{EditInput, handle_edit};
use crate::tools::query::{QueryInput, QueryOutput, handle_query};

#[derive(Clone)]
pub struct GoAnalyzerServer {
    state: Arc<Mutex<ServerState>>,
    tool_router: ToolRouter<Self>,
}

impl GoAnalyzerServer {
    pub fn new(state: ServerState) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl GoAnalyzerServer {
    /// Find and filter Go declarations (functions, methods, structs, interfaces, types)
    /// using a pipeline of select + filters. Returns items with file paths, line numbers,
    /// and signatures.
    #[tool(name = "query")]
    pub async fn query(
        &self,
        Parameters(input): Parameters<QueryInput>,
    ) -> Result<Json<QueryOutput>, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        handle_query(&mut state, &input)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    /// Analyze call relationships between Go symbols. Actions: callers, callees,
    /// reachable_from, dead_code. Returns structured graph data and a readable text tree.
    #[tool(name = "call_graph")]
    pub async fn call_graph(
        &self,
        Parameters(input): Parameters<CallGraphInput>,
    ) -> Result<Json<CallGraphResult>, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        handle_call_graph(&mut state, &input)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    /// Modify Go code using a select + filters pipeline plus an action
    /// (delete, rename, replace_body, add_field, remove_field).
    /// Auto-applies and returns unified diff unless dry_run is true.
    #[tool(name = "edit")]
    pub async fn edit(
        &self,
        Parameters(input): Parameters<EditInput>,
    ) -> Result<Json<EditResult>, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        handle_edit(&mut state, &input)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    /// Get a structural overview of a Go source file: types, functions, methods,
    /// constants, variables with line numbers and optional doc comments.
    #[tool(name = "describe_file")]
    pub async fn describe_file(
        &self,
        Parameters(input): Parameters<DescribeFileInput>,
    ) -> Result<Json<FileOverview>, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        handle_describe_file(&mut state, input)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    /// Get the package tree of the loaded Go module with summary counts per package.
    /// Supports depth limiting for progressive exploration.
    #[tool(name = "describe_module")]
    pub async fn describe_module(
        &self,
        Parameters(input): Parameters<DescribeModuleInput>,
    ) -> Result<Json<ModuleOverview>, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        handle_describe_module(&mut state, input)
            .map(Json)
            .map_err(|e| e.to_string())
    }
}

#[tool_handler]
impl ServerHandler for GoAnalyzerServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "You have access to a Go code analyzer. Typical workflow:\n\
                 1. describe_module - understand the package structure\n\
                 2. describe_file - drill into specific files\n\
                 3. query - find specific types, functions, methods with filters\n\
                 4. call_graph - understand dependencies and call chains\n\
                 5. edit - make changes (returns diff, auto-applies unless dry_run: true)\n\
                 \n\
                 All query/edit tools use a pipeline: select a kind (functions, methods, structs,\n\
                 interfaces, types), then chain filters (named, in_package, exported, on_type,\n\
                 implementing, excluding_tests).",
        )
    }
}
