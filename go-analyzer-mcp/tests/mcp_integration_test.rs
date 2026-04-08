//! End-to-end MCP integration test against mongodb-atlas-cli.
//!
//! Clones the atlas-cli repo at a fixed SHA, starts the MCP server in-process
//! using tokio duplex streams, connects via rmcp client, and exercises all tools.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Once;

use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};

const ATLAS_CLI_REPO: &str = "https://github.com/mongodb/mongodb-atlas-cli.git";
const ATLAS_CLI_SHA: &str = "202610d9607030712d7f6f6efb5b80ab6a3a2084";

static CLONE_ONCE: Once = Once::new();

fn ensure_atlas_cli() -> PathBuf {
    let fixture_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/test-fixtures/mongodb-atlas-cli");

    CLONE_ONCE.call_once(|| {
        if fixture_dir.join(".git").exists() {
            let output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&fixture_dir)
                .output()
                .expect("failed to run git rev-parse");
            let current_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if current_sha == ATLAS_CLI_SHA {
                return;
            }
            let _ = Command::new("git")
                .args(["fetch", "origin", ATLAS_CLI_SHA])
                .current_dir(&fixture_dir)
                .status();
            let _ = Command::new("git")
                .args(["checkout", ATLAS_CLI_SHA])
                .current_dir(&fixture_dir)
                .status();
            return;
        }

        eprintln!("Cloning atlas-cli at {ATLAS_CLI_SHA} (this may take a minute)...");
        std::fs::create_dir_all(fixture_dir.parent().unwrap()).unwrap();

        let status = Command::new("git")
            .args(["clone", "--filter=blob:none", ATLAS_CLI_REPO])
            .arg(&fixture_dir)
            .status()
            .expect("failed to clone atlas-cli");
        assert!(status.success(), "git clone failed");

        let status = Command::new("git")
            .args(["checkout", ATLAS_CLI_SHA])
            .current_dir(&fixture_dir)
            .status()
            .expect("failed to checkout SHA");
        assert!(status.success(), "git checkout failed");
    });

    fixture_dir
}

/// Start the MCP server and client in-process using tokio duplex streams.
/// Returns the client handle.
async fn connect_in_process(
    repo_path: PathBuf,
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    use go_analyzer_mcp::server::GoAnalyzerServer;
    use go_analyzer_mcp::state::ServerState;

    // Create duplex streams: (client_read + server_write) and (server_read + client_write)
    let (client_transport, server_transport) = tokio::io::duplex(65536);

    // Split each duplex stream
    let (server_read, server_write) = tokio::io::split(server_transport);
    let (client_read, client_write) = tokio::io::split(client_transport);

    // Start the server in a background task
    let state = ServerState::new(repo_path);
    let server = GoAnalyzerServer::new(state);
    // Start the server — use waiting() (not cancel()) so it stays alive
    tokio::spawn(async move {
        match server.serve((server_read, server_write)).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => eprintln!("[server] serve error: {e:?}"),
        }
    });

    // Connect the client
    ().serve((client_read, client_write))
        .await
        .expect("failed to initialize MCP client")
}

/// Extract the JSON content from a CallToolResult.
fn extract_json(result: &rmcp::model::CallToolResult) -> Value {
    for content in &result.content {
        if let Some(text) = content.as_text() {
            if let Ok(parsed) = serde_json::from_str::<Value>(&text.text) {
                return parsed;
            }
            return Value::String(text.text.clone());
        }
    }
    panic!("no text content in tool result: {result:?}");
}

/// Normalize paths in JSON for snapshot stability.
fn normalize(value: &Value, repo_root: &str) -> Value {
    match value {
        Value::String(s) => Value::String(s.replace(repo_root, "<REPO>")),
        Value::Array(arr) => Value::Array(arr.iter().map(|v| normalize(v, repo_root)).collect()),
        Value::Object(map) => {
            let normalized: serde_json::Map<String, Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), normalize(v, repo_root)))
                .collect();
            Value::Object(normalized)
        }
        other => other.clone(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mcp_list_tools() {
    let repo_path = ensure_atlas_cli();
    let client = connect_in_process(repo_path).await;

    let tools = client.peer().list_all_tools().await.unwrap();
    let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();

    assert!(tool_names.contains(&"query"), "missing query tool");
    assert!(
        tool_names.contains(&"call_graph"),
        "missing call_graph tool"
    );
    assert!(tool_names.contains(&"edit"), "missing edit tool");
    assert!(
        tool_names.contains(&"describe_file"),
        "missing describe_file tool"
    );
    assert!(
        tool_names.contains(&"describe_module"),
        "missing describe_module tool"
    );
    assert_eq!(
        tool_names.len(),
        5,
        "expected exactly 5 tools, got {tool_names:?}"
    );

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_describe_module() {
    let repo_path = ensure_atlas_cli();
    let client = connect_in_process(repo_path.clone()).await;

    let result = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("describe_module").with_arguments(
                json!({ "depth": 3, "include_docs": false })
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();

    let data = extract_json(&result);
    let repo_root = repo_path
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let normalized = normalize(&data, &repo_root);

    let module = normalized.get("module").and_then(|m| m.as_str()).unwrap();
    assert_eq!(module, "github.com/mongodb/mongodb-atlas-cli/atlascli");

    let packages = normalized
        .get("packages")
        .and_then(|p| p.as_array())
        .unwrap();
    assert!(
        packages.len() > 50,
        "expected many packages, got {}",
        packages.len()
    );

    // Just assert count — full package list is too large and order-dependent for snapshots
    insta::assert_json_snapshot!("describe_module_package_count", packages.len());

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_query_exported_structs_in_store() {
    let repo_path = ensure_atlas_cli();
    let client = connect_in_process(repo_path.clone()).await;

    let result = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("query").with_arguments(
                json!({
                    "select": "structs",
                    "filters": [
                        {"exported": true},
                        {"in_package": "store"},
                        {"excluding_tests": true}
                    ]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();

    let data = extract_json(&result);
    let repo_root = repo_path
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let normalized = normalize(&data, &repo_root);

    let count = normalized.get("count").and_then(|c| c.as_u64()).unwrap();
    assert!(count > 0, "expected exported structs in store package");

    let items = normalized.get("items").and_then(|i| i.as_array()).unwrap();
    let mut summary: Vec<Value> = items
        .iter()
        .map(|item| {
            json!({
                "name": item.get("name").unwrap(),
                "kind": item.get("kind").unwrap(),
                "exported": item.get("exported").unwrap(),
                "signature": item.get("signature").unwrap(),
                "line": item.get("line").unwrap(),
            })
        })
        .collect();
    summary.sort_by_key(|v| v.get("name").unwrap().as_str().unwrap().to_string());

    insta::assert_json_snapshot!("query_exported_structs_store", summary);

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_query_interfaces_in_store() {
    let repo_path = ensure_atlas_cli();
    let client = connect_in_process(repo_path.clone()).await;

    let result = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("query").with_arguments(
                json!({
                    "select": "interfaces",
                    "filters": [
                        {"exported": true},
                        {"in_package": "store"},
                        {"excluding_tests": true}
                    ]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();

    let data = extract_json(&result);
    let repo_root = repo_path
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let normalized = normalize(&data, &repo_root);

    let count = normalized.get("count").and_then(|c| c.as_u64()).unwrap();
    assert!(count > 0, "expected interfaces in store, got {count}");

    let items = normalized.get("items").and_then(|i| i.as_array()).unwrap();
    let mut summary: Vec<Value> = items
        .iter()
        .map(|item| {
            json!({
                "name": item.get("name").unwrap(),
                "line": item.get("line").unwrap(),
            })
        })
        .collect();
    summary.sort_by_key(|v| v.get("name").unwrap().as_str().unwrap().to_string());

    insta::assert_json_snapshot!("query_interfaces_store", summary);

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_describe_file() {
    let repo_path = ensure_atlas_cli();
    let client = connect_in_process(repo_path.clone()).await;

    let file_path = repo_path.join("internal/store/store.go");
    let result = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("describe_file").with_arguments(
                json!({
                    "path": file_path.to_string_lossy(),
                    "include_docs": true
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();

    let data = extract_json(&result);
    let repo_root = repo_path
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let normalized = normalize(&data, &repo_root);

    assert_eq!(
        normalized.get("package").and_then(|p| p.as_str()).unwrap(),
        "store"
    );

    insta::assert_json_snapshot!("describe_file_store", normalized);

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_edit_dry_run() {
    let repo_path = ensure_atlas_cli();
    let client = connect_in_process(repo_path.clone()).await;

    let result = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("edit").with_arguments(
                json!({
                    "select": "functions",
                    "filters": [
                        {"named": "New"},
                        {"in_package": "store"}
                    ],
                    "action": {"rename": "NewRenamed"},
                    "dry_run": true
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();

    let data = extract_json(&result);
    let repo_root = repo_path
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let normalized = normalize(&data, &repo_root);

    let edits = normalized
        .get("edits_applied")
        .and_then(|e| e.as_u64())
        .unwrap();
    assert!(edits > 0, "expected at least one edit");

    let diff = normalized.get("diff").and_then(|d| d.as_str()).unwrap();
    assert!(
        diff.contains("NewRenamed"),
        "diff should contain the new name"
    );

    insta::assert_snapshot!("edit_dry_run_rename", diff);

    client.cancel().await.unwrap();
}
