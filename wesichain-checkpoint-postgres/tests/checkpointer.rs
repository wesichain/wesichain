use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use wesichain_graph::{Checkpoint, Checkpointer, GraphState, StateSchema};

use wesichain_checkpoint_postgres::PostgresCheckpointer;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

fn postgres_database_url() -> String {
    std::env::var("DATABASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .expect("set DATABASE_URL to run postgres integration tests")
}

fn unique_thread_id(prefix: &str) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    format!("{prefix}-{nonce}")
}

#[test]
fn checkpointer_builder_accepts_pool_configuration() {
    let _builder = PostgresCheckpointer::builder("postgres://localhost/example")
        .max_connections(5)
        .min_connections(1)
        .enable_projections(true);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn checkpointer_builder_defaults_projections_disabled() {
    let database_url = postgres_database_url();

    let checkpointer = PostgresCheckpointer::builder(database_url)
        .max_connections(5)
        .build()
        .await
        .expect("postgres checkpointer should build");

    assert!(!checkpointer.projections_enabled());
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn checkpointer_trait_round_trip_save_and_load() {
    let database_url = postgres_database_url();

    let checkpointer = PostgresCheckpointer::builder(database_url)
        .max_connections(5)
        .min_connections(1)
        .enable_projections(true)
        .build()
        .await
        .expect("postgres checkpointer should build");

    let thread_id = unique_thread_id("pg-thread");
    let checkpoint = Checkpoint::new(
        thread_id.clone(),
        GraphState::new(DemoState { count: 7 }),
        3,
        "node-a".to_string(),
        vec![("node-b".to_string(), 4)],
    );

    checkpointer
        .save(&checkpoint)
        .await
        .expect("checkpoint should save");

    let loaded: Checkpoint<DemoState> = checkpointer
        .load(&thread_id)
        .await
        .expect("checkpoint should load")
        .expect("checkpoint should exist");

    assert_eq!(loaded.thread_id, thread_id);
    assert_eq!(loaded.step, 3);
    assert_eq!(loaded.node, "node-a");
    assert_eq!(loaded.state.data.count, 7);
    assert_eq!(loaded.queue, vec![("node-b".to_string(), 4)]);
}
