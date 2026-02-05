use serde::{Deserialize, Serialize};
use wesichain_graph::{GraphBuilder, GraphError, GraphState, StateSchema};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

#[tokio::test]
async fn graph_returns_missing_node_error() {
    let graph = GraphBuilder::new().set_entry("missing").build();
    let state = GraphState::new(DemoState { count: 0 });
    let err = graph.invoke_graph(state).await.unwrap_err();
    assert!(matches!(err, GraphError::MissingNode { .. }));
}
