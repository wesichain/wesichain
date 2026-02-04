use futures::StreamExt;
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{GraphBuilder, GraphState, StateSchema, StateUpdate, START};

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
        .set_entry("inc")
        .build_program();

    assert!(program.name_to_index.contains_key("inc"));
    assert!(program.name_to_index.contains_key("inc2"));
    assert!(!program.name_to_index.contains_key(START));
    assert_eq!(program.graph.node_count(), 2);
    assert_eq!(program.graph.edge_count(), 1);

    let inc_index = program.name_to_index["inc"];
    let inc2_index = program.name_to_index["inc2"];
    let edges: Vec<_> = program
        .graph
        .edge_references()
        .map(|edge| (edge.source(), edge.target()))
        .collect();
    assert_eq!(edges, vec![(inc_index, inc2_index)]);
}
