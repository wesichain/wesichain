use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{
    ExecutionConfig, ExecutionOptions, GraphBuilder, GraphState, StateSchema, StateUpdate,
};

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
async fn graph_enforces_max_steps() {
    let graph = GraphBuilder::new()
        .add_node("inc", Inc)
        .add_edge("inc", "inc")
        .set_entry("inc")
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    let options = ExecutionOptions {
        max_steps: Some(2),
        cycle_detection: Some(false),
        cycle_window: None,
    };
    let err = graph.invoke_with_options(state, options).await.unwrap_err();
    assert!(err.to_string().contains("Max steps exceeded"));
}

#[tokio::test]
async fn graph_detects_cycle_in_recent_window() {
    let graph = GraphBuilder::new()
        .add_node("inc", Inc)
        .add_edge("inc", "inc")
        .set_entry("inc")
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    let options = ExecutionOptions {
        max_steps: Some(10),
        cycle_detection: Some(true),
        cycle_window: Some(2),
    };
    let err = graph.invoke_with_options(state, options).await.unwrap_err();
    assert!(err.to_string().contains("Cycle detected"));
}

#[tokio::test]
async fn graph_options_override_defaults() {
    let graph = GraphBuilder::new()
        .with_default_config(ExecutionConfig {
            max_steps: Some(1),
            cycle_detection: true,
            cycle_window: 2,
        })
        .add_node("one", Inc)
        .add_node("two", Inc)
        .add_edge("one", "two")
        .set_entry("one")
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    let options = ExecutionOptions {
        max_steps: Some(5),
        cycle_detection: Some(false),
        cycle_window: None,
    };
    let out = graph.invoke_with_options(state, options).await.unwrap();
    assert_eq!(out.data.count, 2);
}
