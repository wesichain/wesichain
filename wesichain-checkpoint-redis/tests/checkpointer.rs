use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use wesichain_checkpoint_redis::RedisCheckpointer;
use wesichain_graph::{
    Checkpoint, CheckpointMetadata, Checkpointer, GraphState, HistoryCheckpointer, StateSchema,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {
    type Update = Self;
    fn apply(_: &Self, update: Self) -> Self {
        update
    }
}

fn redis_test_url() -> String {
    std::env::var("REDIS_TEST_URL")
        .expect("REDIS_TEST_URL must be set to run Redis integration tests")
}

fn unique_namespace(prefix: &str) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    format!("{prefix}-{nonce}")
}

#[tokio::test]
#[ignore = "requires REDIS_TEST_URL"]
async fn save_and_load_roundtrip() {
    let checkpointer = RedisCheckpointer::new(&redis_test_url(), unique_namespace("redis-rt"))
        .await
        .expect("redis checkpointer should connect");

    let checkpoint = Checkpoint::new(
        "thread-1".to_string(),
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
        .load("thread-1")
        .await
        .expect("checkpoint should load")
        .expect("checkpoint should exist");

    assert_eq!(loaded.thread_id, "thread-1");
    assert_eq!(loaded.step, 3);
    assert_eq!(loaded.node, "node-a");
    assert_eq!(loaded.state.data.count, 7);
    assert_eq!(loaded.queue, vec![("node-b".to_string(), 4)]);
}

#[tokio::test]
#[ignore = "requires REDIS_TEST_URL"]
async fn load_missing_thread_returns_none() {
    let checkpointer = RedisCheckpointer::new(&redis_test_url(), unique_namespace("redis-miss"))
        .await
        .expect("redis checkpointer should connect");

    let loaded: Option<Checkpoint<DemoState>> = checkpointer
        .load("thread-does-not-exist")
        .await
        .expect("load should succeed");

    assert!(loaded.is_none());
}

#[tokio::test]
#[ignore = "requires REDIS_TEST_URL"]
async fn concurrent_saves_produce_monotonic_history() {
    let checkpointer = RedisCheckpointer::new(&redis_test_url(), unique_namespace("redis-conc"))
        .await
        .expect("redis checkpointer should connect");

    let thread_id = "thread-concurrent".to_string();
    let worker_a = {
        let checkpointer = checkpointer.clone();
        let thread_id = thread_id.clone();
        tokio::spawn(async move {
            for i in 0..25 {
                let checkpoint = Checkpoint::new(
                    thread_id.clone(),
                    GraphState::new(DemoState { count: i }),
                    i as u64,
                    "node-a".to_string(),
                    vec![],
                );
                checkpointer
                    .save(&checkpoint)
                    .await
                    .expect("worker A checkpoint should save");
            }
        })
    };

    let worker_b = {
        let checkpointer = checkpointer.clone();
        let thread_id = thread_id.clone();
        tokio::spawn(async move {
            for i in 25..50 {
                let checkpoint = Checkpoint::new(
                    thread_id.clone(),
                    GraphState::new(DemoState { count: i }),
                    i as u64,
                    "node-b".to_string(),
                    vec![],
                );
                checkpointer
                    .save(&checkpoint)
                    .await
                    .expect("worker B checkpoint should save");
            }
        })
    };

    worker_a.await.expect("worker A should finish");
    worker_b.await.expect("worker B should finish");

    let history: Vec<CheckpointMetadata> =
        <RedisCheckpointer as HistoryCheckpointer<DemoState>>::list_checkpoints(
            &checkpointer,
            &thread_id,
        )
        .await
        .expect("history should load");

    assert_eq!(history.len(), 50);
    for (idx, checkpoint) in history.iter().enumerate() {
        assert_eq!(checkpoint.seq as usize, idx + 1);
    }
}
