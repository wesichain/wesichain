use serde::{Deserialize, Serialize};
use tempfile::tempdir;
use wesichain_graph::{
    Checkpoint, Checkpointer, FileCheckpointer, GraphState, HistoryCheckpointer, StateSchema,
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

#[tokio::test]
async fn file_checkpointer_appends_and_loads_latest() {
    let dir = tempdir().unwrap();
    let checkpointer = FileCheckpointer::new(dir.path());

    let first = Checkpoint::new("thread/1".to_string(), GraphState::new(DemoState { count: 1 }));
    let second = Checkpoint::new("thread/1".to_string(), GraphState::new(DemoState { count: 2 }));

    checkpointer.save(&first).await.unwrap();
    checkpointer.save(&second).await.unwrap();

    let loaded: Checkpoint<DemoState> = checkpointer.load("thread/1").await.unwrap().unwrap();
    assert_eq!(loaded.state.data.count, 2);

    let path = dir.path().join("thread_1.jsonl");
    assert!(path.exists());
}

#[tokio::test]
async fn file_checkpointer_lists_metadata() {
    let dir = tempdir().unwrap();
    let checkpointer = FileCheckpointer::new(dir.path());

    let first = Checkpoint::new("thread-2".to_string(), GraphState::new(DemoState { count: 1 }));
    let second = Checkpoint::new("thread-2".to_string(), GraphState::new(DemoState { count: 2 }));

    checkpointer.save(&first).await.unwrap();
    checkpointer.save(&second).await.unwrap();

    let history =
        <FileCheckpointer as HistoryCheckpointer<DemoState>>::list_checkpoints(&checkpointer, "thread-2")
            .await
            .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].seq, 1);
    assert_eq!(history[1].seq, 2);
    assert!(!history[0].created_at.is_empty());
}
