use futures::StreamExt;
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{GraphBuilder, GraphInterrupt, GraphState, StateSchema, StateUpdate};

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

#[tokio::test]
async fn graph_interrupts_before_node() {
    let graph = GraphBuilder::new()
        .add_node("inc", Inc)
        .set_entry("inc")
        .with_interrupt_before(["inc"])
        .build();

    let _ = std::mem::size_of::<GraphInterrupt<DemoState>>();

    let state = GraphState::new(DemoState { count: 0 });
    let result = graph.invoke_graph(state).await;
    assert!(matches!(result, Err(wesichain_graph::GraphError::Interrupted)));
}
