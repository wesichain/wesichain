use serde::{Deserialize, Serialize};
use wesichain_graph::{Checkpoint, Checkpointer, GraphState, InMemoryCheckpointer, StateSchema};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

#[tokio::test]
async fn checkpointer_roundtrip() {
    let checkpointer = InMemoryCheckpointer::default();
    let state = GraphState::new(DemoState { count: 1 });
    let checkpoint = Checkpoint::new("thread-1".to_string(), state);
    checkpointer.save(&checkpoint).await.unwrap();
    let loaded = checkpointer.load("thread-1").await.unwrap();
    assert_eq!(loaded.unwrap().state.data.count, 1);
}
