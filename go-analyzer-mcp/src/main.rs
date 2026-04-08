use clap::Parser;
use rmcp::ServiceExt;
use std::path::PathBuf;

use go_analyzer_mcp::server::GoAnalyzerServer;
use go_analyzer_mcp::state::ServerState;

#[derive(Parser)]
#[command(name = "go-analyzer-mcp", about = "MCP server for Go code analysis")]
struct Cli {
    /// Path to the Go project to analyze. Defaults to current directory.
    #[arg(long, default_value = ".")]
    path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let path = cli.path.canonicalize()?;
    eprintln!("go-analyzer-mcp: serving {}", path.display());

    let state = ServerState::new(path);
    let server = GoAnalyzerServer::new(state);
    let running = server.serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;

    Ok(())
}
