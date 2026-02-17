use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{
    Checkpointer, ExecutionOptions, GraphBuilder, GraphState, InMemoryCheckpointer, StateSchema,
    StateUpdate,
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

struct AddOne;

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

#[tokio::test]
async fn graph_saves_checkpoint_each_step() {
    let checkpointer = InMemoryCheckpointer::default();
    let graph = GraphBuilder::new()
        .add_node("one", AddOne)
        .set_entry("one")
        .with_checkpointer(checkpointer.clone(), "thread-1")
        .build();

    let state = GraphState::new(DemoState { count: 1 });
    let out = graph.invoke(state).await.unwrap();
    assert_eq!(out.data.count, 2);

    let loaded = checkpointer.load("thread-1").await.unwrap().unwrap();
    assert_eq!(loaded.state.data.count, 2);
}

#[tokio::test]
async fn graph_uses_invocation_checkpoint_thread_override() {
    let checkpointer = InMemoryCheckpointer::default();
    let graph = GraphBuilder::new()
        .add_node("one", AddOne)
        .set_entry("one")
        .with_checkpointer(checkpointer.clone(), "builder-thread")
        .build();

    let _ = graph
        .invoke_graph_with_options(
            GraphState::new(DemoState { count: 0 }),
            ExecutionOptions {
                checkpoint_thread_id: Some("override-thread".to_string()),
                ..ExecutionOptions::default()
            },
        )
        .await
        .expect("graph invocation should succeed");

    assert!(checkpointer
        .load("builder-thread")
        .await
        .expect("builder thread load should succeed")
        .is_none());

    assert!(checkpointer
        .load("override-thread")
        .await
        .expect("override thread load should succeed")
        .is_some());
}

#[tokio::test]
async fn graph_auto_resume_loads_latest_state_before_execution() {
    let checkpointer = InMemoryCheckpointer::default();
    let graph = GraphBuilder::new()
        .add_node("one", AddOne)
        .set_entry("one")
        .with_checkpointer(checkpointer.clone(), "builder-thread")
        .build();

    let first = graph
        .invoke_graph_with_options(
            GraphState::new(DemoState { count: 1 }),
            ExecutionOptions {
                checkpoint_thread_id: Some("resume-thread".to_string()),
                ..ExecutionOptions::default()
            },
        )
        .await
        .expect("first invocation should succeed");
    assert_eq!(first.data.count, 2);

    let resumed = graph
        .invoke_graph_with_options(
            GraphState::new(DemoState { count: 0 }),
            ExecutionOptions {
                checkpoint_thread_id: Some("resume-thread".to_string()),
                auto_resume: true,
                ..ExecutionOptions::default()
            },
        )
        .await
        .expect("resumed invocation should succeed");

    assert_eq!(resumed.data.count, 3);
}

#[tokio::test]
async fn graph_auto_resume_without_checkpoint_starts_from_input_state() {
    let checkpointer = InMemoryCheckpointer::default();
    let graph = GraphBuilder::new()
        .add_node("one", AddOne)
        .set_entry("one")
        .with_checkpointer(checkpointer, "builder-thread")
        .build();

    let out = graph
        .invoke_graph_with_options(
            GraphState::new(DemoState { count: 0 }),
            ExecutionOptions {
                checkpoint_thread_id: Some("fresh-thread".to_string()),
                auto_resume: true,
                ..ExecutionOptions::default()
            },
        )
        .await
        .expect("graph invocation should succeed");

    assert_eq!(out.data.count, 1);
}
