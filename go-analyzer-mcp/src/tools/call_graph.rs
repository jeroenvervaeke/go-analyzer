use go_analyzer::callgraph::{CallGraph, Symbol, SymbolKind};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::output::{CallGraphEdge, CallGraphNode, CallGraphResult};
use crate::state::ServerState;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CallGraphInput {
    pub action: CallGraphAction,
    #[serde(default)]
    pub symbol: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallGraphAction {
    Callers,
    Callees,
    ReachableFrom,
    DeadCode,
}

#[derive(Debug, thiserror::Error)]
pub enum CallGraphError {
    #[error("state error: {0}")]
    State(String),
    #[error("symbol not found: {0}")]
    SymbolNotFound(String),
    #[error("{0} action requires a symbol parameter")]
    MissingSymbol(&'static str),
}

pub fn handle_call_graph(
    state: &mut ServerState,
    input: &CallGraphInput,
) -> Result<CallGraphResult, CallGraphError> {
    let repo = state
        .repo()
        .map_err(|e| CallGraphError::State(e.to_string()))?;
    let graph = CallGraph::build(repo);

    match input.action {
        CallGraphAction::Callers => {
            let name = input
                .symbol
                .as_deref()
                .ok_or(CallGraphError::MissingSymbol("callers"))?;
            callers(&graph, name)
        }
        CallGraphAction::Callees => {
            let name = input
                .symbol
                .as_deref()
                .ok_or(CallGraphError::MissingSymbol("callees"))?;
            callees(&graph, name)
        }
        CallGraphAction::ReachableFrom => {
            let name = input
                .symbol
                .as_deref()
                .ok_or(CallGraphError::MissingSymbol("reachable_from"))?;
            reachable_from(&graph, name)
        }
        CallGraphAction::DeadCode => dead_code(&graph),
    }
}

fn callers(graph: &CallGraph, symbol_name: &str) -> Result<CallGraphResult, CallGraphError> {
    let targets = find_symbols_by_name(graph, symbol_name);
    if targets.is_empty() {
        return Err(CallGraphError::SymbolNotFound(symbol_name.to_owned()));
    }

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut text = String::new();

    for target in &targets {
        text.push_str(&target.name);
        text.push('\n');

        let Some(callers_set) = graph.called_by.get(target) else {
            continue;
        };

        let mut sorted_callers: Vec<&Symbol> = callers_set.iter().collect();
        sorted_callers.sort_by_key(|s| s.to_string());

        for caller in sorted_callers {
            let Some(entry) = graph.symbols.get(caller) else {
                continue;
            };
            let location = format!("{}:{}", entry.file.display(), entry.span.start_row + 1);
            text.push_str(&format!("  <- {} ({location})\n", caller.name));

            nodes.push(CallGraphNode {
                symbol: caller.to_string(),
                file: entry.file.clone(),
                line: entry.span.start_row + 1,
            });
            edges.push(CallGraphEdge {
                from: caller.to_string(),
                to: target.to_string(),
            });
        }
    }

    // Add target nodes themselves
    for target in &targets {
        if let Some(entry) = graph.symbols.get(target) {
            nodes.push(CallGraphNode {
                symbol: target.to_string(),
                file: entry.file.clone(),
                line: entry.span.start_row + 1,
            });
        }
    }

    Ok(CallGraphResult { nodes, edges, text })
}

fn callees(graph: &CallGraph, symbol_name: &str) -> Result<CallGraphResult, CallGraphError> {
    let sources = find_symbols_by_name(graph, symbol_name);
    if sources.is_empty() {
        return Err(CallGraphError::SymbolNotFound(symbol_name.to_owned()));
    }

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut text = String::new();

    for source in &sources {
        text.push_str(&source.name);
        text.push('\n');

        let Some(callees_set) = graph.calls.get(source) else {
            continue;
        };

        let mut sorted_callees: Vec<&Symbol> = callees_set.iter().collect();
        sorted_callees.sort_by_key(|s| s.to_string());

        for callee in sorted_callees {
            let Some(entry) = graph.symbols.get(callee) else {
                continue;
            };
            let location = format!("{}:{}", entry.file.display(), entry.span.start_row + 1);
            text.push_str(&format!("  -> {} ({location})\n", callee.name));

            nodes.push(CallGraphNode {
                symbol: callee.to_string(),
                file: entry.file.clone(),
                line: entry.span.start_row + 1,
            });
            edges.push(CallGraphEdge {
                from: source.to_string(),
                to: callee.to_string(),
            });
        }
    }

    // Add source nodes themselves
    for source in &sources {
        if let Some(entry) = graph.symbols.get(source) {
            nodes.push(CallGraphNode {
                symbol: source.to_string(),
                file: entry.file.clone(),
                line: entry.span.start_row + 1,
            });
        }
    }

    Ok(CallGraphResult { nodes, edges, text })
}

fn reachable_from(graph: &CallGraph, symbol_name: &str) -> Result<CallGraphResult, CallGraphError> {
    let entries_syms = find_symbols_by_name(graph, symbol_name);
    if entries_syms.is_empty() {
        return Err(CallGraphError::SymbolNotFound(symbol_name.to_owned()));
    }

    let entry_syms_owned: Vec<Symbol> = entries_syms.iter().map(|s| (*s).clone()).collect();
    let reachable = graph.reachable_from(&entry_syms_owned);

    let mut nodes: Vec<CallGraphNode> = reachable
        .iter()
        .filter_map(|sym| {
            graph.symbols.get(sym).map(|entry| CallGraphNode {
                symbol: sym.to_string(),
                file: entry.file.clone(),
                line: entry.span.start_row + 1,
            })
        })
        .collect();
    nodes.sort_by_key(|n| n.symbol.clone());

    let edges: Vec<CallGraphEdge> = graph
        .edges
        .iter()
        .filter(|e| reachable.contains(&e.caller) && reachable.contains(&e.callee))
        .map(|e| CallGraphEdge {
            from: e.caller.to_string(),
            to: e.callee.to_string(),
        })
        .collect();

    let mut text = format!("Reachable from {symbol_name}:\n");
    for node in &nodes {
        text.push_str(&format!(
            "  {} ({}:{})\n",
            node.symbol,
            node.file.display(),
            node.line
        ));
    }

    Ok(CallGraphResult { nodes, edges, text })
}

fn dead_code(graph: &CallGraph) -> Result<CallGraphResult, CallGraphError> {
    // Entry points: exported symbols, main, init, and types with serde tags
    let entries: Vec<Symbol> = graph
        .symbols
        .iter()
        .filter_map(|(sym, entry)| {
            if entry.exported
                || sym.name == "main"
                || sym.name == "init"
                || entry.kind == SymbolKind::Type
            {
                Some(sym.clone())
            } else {
                None
            }
        })
        .collect();

    let reachable = graph.reachable_from(&entries);

    let mut dead: Vec<&go_analyzer::callgraph::SymbolEntry> = graph
        .symbols
        .values()
        .filter(|entry| !reachable.contains(&entry.symbol))
        .collect();
    dead.sort_by_key(|e| e.symbol.to_string());

    let nodes: Vec<CallGraphNode> = dead
        .iter()
        .map(|entry| CallGraphNode {
            symbol: entry.symbol.to_string(),
            file: entry.file.clone(),
            line: entry.span.start_row + 1,
        })
        .collect();

    let mut text = String::from("Dead code (unreachable symbols):\n");
    for node in &nodes {
        text.push_str(&format!(
            "  {} ({}:{})\n",
            node.symbol,
            node.file.display(),
            node.line
        ));
    }

    Ok(CallGraphResult {
        nodes,
        edges: Vec::new(),
        text,
    })
}

fn find_symbols_by_name<'a>(graph: &'a CallGraph, name: &str) -> Vec<&'a Symbol> {
    graph.symbols.keys().filter(|s| s.name == name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_state() -> ServerState {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../go-analyzer/tests/fixture_repo");
        ServerState::new(path.canonicalize().unwrap())
    }

    #[test]
    fn test_callees_of_known_function() {
        let mut state = fixture_state();
        let input = CallGraphInput {
            action: CallGraphAction::Callees,
            symbol: Some("RunServer".to_owned()),
        };

        let result = handle_call_graph(&mut state, &input).unwrap();

        // RunServer calls Server.Start(), so expect at least one callee
        assert!(
            !result.nodes.is_empty(),
            "expected at least one callee node for RunServer"
        );
        assert!(
            !result.edges.is_empty(),
            "expected at least one callee edge for RunServer"
        );
    }

    #[test]
    fn test_dead_code_finds_unreachable() {
        let mut state = fixture_state();
        let input = CallGraphInput {
            action: CallGraphAction::DeadCode,
            symbol: None,
        };

        let result = handle_call_graph(&mut state, &input).unwrap();

        // helperFunc is unexported and uncalled, should appear in dead code
        let found = result.nodes.iter().any(|n| n.symbol.contains("helperFunc"));
        assert!(
            found,
            "expected helperFunc in dead code results, got: {:?}",
            result.nodes.iter().map(|n| &n.symbol).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_callers_of_nonexistent_returns_error() {
        let mut state = fixture_state();
        let input = CallGraphInput {
            action: CallGraphAction::Callers,
            symbol: Some("nonexistent_xyz".to_owned()),
        };

        let err = handle_call_graph(&mut state, &input).unwrap_err();
        assert!(
            matches!(err, CallGraphError::SymbolNotFound(_)),
            "expected SymbolNotFound, got: {err}"
        );
    }

    #[test]
    fn test_missing_symbol_returns_error() {
        let mut state = fixture_state();
        let input = CallGraphInput {
            action: CallGraphAction::Callers,
            symbol: None,
        };

        let err = handle_call_graph(&mut state, &input).unwrap_err();
        assert!(
            matches!(err, CallGraphError::MissingSymbol(_)),
            "expected MissingSymbol, got: {err}"
        );
    }
}
