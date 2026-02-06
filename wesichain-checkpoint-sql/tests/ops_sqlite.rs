use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;
use wesichain_checkpoint_sql::migrations::run_migrations;
use wesichain_checkpoint_sql::ops::{load_latest_checkpoint, save_checkpoint};

#[tokio::test]
async fn ops_sqlite_migration_bootstrap_creates_tables() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should connect");

    run_migrations(&pool)
        .await
        .expect("migrations should bootstrap schema");

    let table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('checkpoints', 'sessions', 'messages', 'graph_triples')",
    )
    .fetch_one(&pool)
    .await
    .expect("table count query should run");

    assert_eq!(table_count, 4);
}

#[tokio::test]
async fn ops_sqlite_save_assigns_seq_per_thread() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should connect");

    run_migrations(&pool)
        .await
        .expect("migrations should bootstrap schema");

    let first = save_checkpoint(
        &pool,
        "thread-a",
        "n1",
        1,
        "2026-02-06T00:00:00Z",
        &serde_json::json!({"count": 1}),
    )
    .await
    .expect("first checkpoint should save");
    let second = save_checkpoint(
        &pool,
        "thread-a",
        "n2",
        2,
        "2026-02-06T00:00:01Z",
        &serde_json::json!({"count": 2}),
    )
    .await
    .expect("second checkpoint should save");
    let other_thread = save_checkpoint(
        &pool,
        "thread-b",
        "n1",
        1,
        "2026-02-06T00:00:02Z",
        &serde_json::json!({"count": 3}),
    )
    .await
    .expect("checkpoint for another thread should save");

    assert_eq!(first, 1);
    assert_eq!(second, 2);
    assert_eq!(other_thread, 1);

    let seqs: Vec<i64> = sqlx::query("SELECT seq FROM checkpoints WHERE thread_id = ? ORDER BY seq")
        .bind("thread-a")
        .fetch_all(&pool)
        .await
        .expect("seq query should run")
        .into_iter()
        .map(|row| row.get("seq"))
        .collect();
    assert_eq!(seqs, vec![1, 2]);
}

#[tokio::test]
async fn ops_sqlite_load_returns_latest_checkpoint_only() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should connect");

    run_migrations(&pool)
        .await
        .expect("migrations should bootstrap schema");

    save_checkpoint(
        &pool,
        "thread-a",
        "n1",
        1,
        "2026-02-06T00:00:00Z",
        &serde_json::json!({"rev": 1}),
    )
    .await
    .expect("first checkpoint should save");
    save_checkpoint(
        &pool,
        "thread-a",
        "n2",
        2,
        "2026-02-06T00:00:01Z",
        &serde_json::json!({"rev": 2}),
    )
    .await
    .expect("second checkpoint should save");

    let latest = load_latest_checkpoint(&pool, "thread-a")
        .await
        .expect("load should succeed")
        .expect("latest checkpoint should exist");

    assert_eq!(latest.seq, 2);
    assert_eq!(latest.node.as_deref(), Some("n2"));
    assert_eq!(latest.step, Some(2));
    assert_eq!(latest.created_at, "2026-02-06T00:00:01Z");
    assert_eq!(latest.state_json, serde_json::json!({"rev": 2}));
}
