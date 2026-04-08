use schemars::JsonSchema;
use serde::Deserialize;

use crate::output::QueryItem;
use crate::selection_builder::{Filter, SelectKind, build_query};
use crate::state::ServerState;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryInput {
    pub select: SelectKind,
    #[serde(default)]
    pub filters: Vec<Filter>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct QueryOutput {
    pub items: Vec<QueryItem>,
    pub count: usize,
}

pub fn handle_query(
    state: &mut ServerState,
    input: &QueryInput,
) -> Result<QueryOutput, QueryError> {
    let repo = state.repo().map_err(|e| QueryError::State(e.to_string()))?;
    let items = build_query(repo, &input.select, &input.filters);
    let count = items.len();
    Ok(QueryOutput { items, count })
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("state error: {0}")]
    State(String),
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
    fn test_handle_query_returns_items() {
        let mut state = fixture_state();
        let input = QueryInput {
            select: SelectKind::Structs,
            filters: vec![],
        };

        let output = handle_query(&mut state, &input).unwrap();

        assert!(output.count > 0, "expected at least one struct");
        assert_eq!(
            output.count,
            output.items.len(),
            "count must match items.len()"
        );
    }

    #[test]
    fn test_handle_query_with_filters() {
        let mut state = fixture_state();
        let input = QueryInput {
            select: SelectKind::Methods,
            filters: vec![Filter::OnType("Server".to_owned())],
        };

        let output = handle_query(&mut state, &input).unwrap();

        assert!(!output.items.is_empty(), "expected methods on Server");
        for item in &output.items {
            let recv = item
                .receiver
                .as_ref()
                .expect("method should have a receiver");
            assert!(
                recv.contains("Server"),
                "receiver should contain Server, got: {recv}"
            );
        }
    }
}
