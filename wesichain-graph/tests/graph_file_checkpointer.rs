use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use tempfile::tempdir;
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{FileCheckpointer, GraphBuilder, GraphState, StateSchema, StateUpdate};

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
async fn graph_writes_checkpoint_history_to_file() {
    let dir = tempdir().unwrap();
    let checkpointer = FileCheckpointer::new(dir.path());
    let graph = GraphBuilder::new()
        .add_node("one", Inc)
        .add_node("two", Inc)
        .add_edge("one", "two")
        .set_entry("one")
        .with_checkpointer(checkpointer, "thread-1")
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    let out = graph.invoke(state).await.unwrap();
    assert_eq!(out.data.count, 2);

    let path = dir.path().join("thread-1.jsonl");
    let contents = std::fs::read_to_string(path).unwrap();
    assert_eq!(contents.lines().count(), 2);
}
