use futures::StreamExt;
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{GraphBuilder, GraphState, StateSchema, StateUpdate, END, START};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

struct Inc;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for Inc {
    async fn invoke(
        &self,
        input: GraphState<DemoState>,
    ) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState {
            count: input.data.count + 1,
        }))
    }

    fn stream(
        &self,
        _input: GraphState<DemoState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[test]
fn graph_program_skips_start_edges() {
    let program = GraphBuilder::new()
        .add_node("inc", Inc)
        .add_node("inc2", Inc)
        .add_edge(START, "inc")
        .add_edge("inc", "inc2")
        .add_edge("inc2", END)
        .add_edge("inc2", "missing")
        .set_entry("inc")
        .build_program();

    let mut nodes = program.node_names();
    nodes.sort();
    assert_eq!(nodes, vec!["inc".to_string(), "inc2".to_string()]);
    assert!(!nodes.contains(&START.to_string()));
    assert!(!nodes.contains(&END.to_string()));

    let mut edges = program.edge_names();
    edges.sort();
    assert_eq!(edges, vec![("inc".to_string(), "inc2".to_string())]);
}
