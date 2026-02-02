use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{GraphBuilder, GraphState, StateSchema, StateUpdate};

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
async fn graph_conditional_routes_by_state() {
    let graph = GraphBuilder::new()
        .add_node("inc", Inc)
        .add_node("inc2", Inc)
        .add_node("stop", Inc)
        .add_conditional_edge("inc", |state: &GraphState<DemoState>| {
            if state.data.count > 1 {
                "stop".to_string()
            } else {
                "inc2".to_string()
            }
        })
        .set_entry("inc")
        .build();

    let state = GraphState::new(DemoState { count: 1 });
    let out = graph.invoke(state).await.unwrap();
    assert_eq!(out.data.count, 3);
}
