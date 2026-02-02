use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{GraphBuilder, GraphState, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

struct AddOne;
struct AddTwo;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for AddOne {
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

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for AddTwo {
    async fn invoke(
        &self,
        input: GraphState<DemoState>,
    ) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState {
            count: input.data.count + 2,
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
async fn graph_executes_edges_in_order() {
    let graph = GraphBuilder::new()
        .add_node("one", AddOne)
        .add_node("two", AddTwo)
        .add_edge("one", "two")
        .set_entry("one")
        .build();

    let state = GraphState::new(DemoState { count: 1 });
    let out = graph.invoke(state).await.unwrap();
    assert_eq!(out.data.count, 4);
}
