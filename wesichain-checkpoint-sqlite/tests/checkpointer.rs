use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
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

#[tokio::test]
async fn checkpointer_load_fails_when_required_columns_are_null() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    let db_path = std::env::temp_dir().join(format!("checkpoint-null-{nonce}.db"));
    std::fs::File::create(&db_path).expect("db file should be creatable");
    let database_url = format!("sqlite://{}", db_path.display());

    let checkpointer = SqliteCheckpointer::builder(database_url.clone())
        .max_connections(1)
        .build()
        .await
        .expect("sqlite checkpointer should build");

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("direct sqlite pool should connect");

    sqlx::query(
        "INSERT INTO checkpoints (thread_id, seq, created_at, node, step, state_json) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("thread-null")
    .bind(1_i64)
    .bind("2026-02-06T00:00:00Z")
    .bind(Option::<String>::None)
    .bind(Option::<i64>::None)
    .bind("{\"data\":{\"count\":1}}")
    .execute(&pool)
    .await
    .expect("seed row should insert");

    let load_result: Result<Option<Checkpoint<DemoState>>, _> = checkpointer.load("thread-null").await;
    let error = load_result.expect_err("load should fail when required columns are null");
    assert!(error.to_string().contains("checkpoint step is missing"));

    drop(pool);
    let _ = std::fs::remove_file(db_path);
}
