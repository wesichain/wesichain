use serde::{Deserialize, Serialize};
use wesichain_graph::{Checkpoint, Checkpointer, GraphState, StateSchema};

use wesichain_checkpoint_sqlite::SqliteCheckpointer;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

#[tokio::test]
async fn checkpointer_builder_defaults_projections_disabled() {
    let checkpointer = SqliteCheckpointer::builder("sqlite::memory:")
        .max_connections(1)
        .build()
        .await
        .expect("sqlite checkpointer should build");

    assert!(!checkpointer.projections_enabled());
}

#[tokio::test]
async fn checkpointer_trait_round_trip_save_and_load() {
    let checkpointer = SqliteCheckpointer::builder("sqlite::memory:")
        .max_connections(1)
        .build()
        .await
        .expect("sqlite checkpointer should build");

    let checkpoint = Checkpoint::new(
        "thread-1".to_string(),
        GraphState::new(DemoState { count: 7 }),
        3,
        "node-a".to_string(),
    );

    checkpointer
        .save(&checkpoint)
        .await
        .expect("checkpoint should save");

    let loaded: Checkpoint<DemoState> = checkpointer
        .load("thread-1")
        .await
        .expect("checkpoint should load")
        .expect("checkpoint should exist");

    assert_eq!(loaded.thread_id, "thread-1");
    assert_eq!(loaded.step, 3);
    assert_eq!(loaded.node, "node-a");
    assert_eq!(loaded.state.data.count, 7);
}
