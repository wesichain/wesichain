use sqlx::Row;
use wesichain_checkpoint_sql::migrations::{run_migrations, run_migrations_in_transaction};
use wesichain_checkpoint_sql::ops::{
    load_latest_checkpoint, save_checkpoint, save_checkpoint_in_transaction,
};

#[test]
#[allow(clippy::let_underscore_future)]
fn ops_api_accepts_postgres_pool_type() {
    fn assert_backend_agnostic<DB>(pool: &sqlx::Pool<DB>)
    where
        DB: sqlx::Database,
        for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
        for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
        for<'q> String: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
        for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
        for<'c> &'c sqlx::Pool<DB>: sqlx::Executor<'c, Database = DB>,
        for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
        for<'r> String: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
        for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
        for<'r> Option<String>: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
        for<'r> Option<i64>: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
        &'static str: sqlx::ColumnIndex<DB::Row>,
        usize: sqlx::ColumnIndex<DB::Row>,
    {
        let _ = run_migrations(pool);
        let _ = save_checkpoint(
            pool,
            "thread-a",
            "n1",
            1,
            "2026-02-06T00:00:00Z",
            &serde_json::json!({"count": 1}),
        );
        let _ = load_latest_checkpoint(pool, "thread-a");
    }

    let _ = assert_backend_agnostic::<sqlx::Postgres> as fn(&sqlx::Pool<sqlx::Postgres>);
    let _ = assert_backend_agnostic::<sqlx::Sqlite> as fn(&sqlx::Pool<sqlx::Sqlite>);
}

async fn sqlite_pool() -> sqlx::SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should connect")
}

#[tokio::test]
async fn ops_sqlite_migration_bootstrap_creates_tables() {
    let pool = sqlite_pool().await;

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
async fn ops_sqlite_migrations_can_run_in_transaction_context() {
    let pool = sqlite_pool().await;

    let mut tx = pool
        .begin()
        .await
        .expect("transaction should begin for migrations");

    run_migrations_in_transaction(&mut tx)
        .await
        .expect("migrations should run inside transaction");

    let table_count_in_tx: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('checkpoints', 'sessions', 'messages', 'graph_triples')",
    )
    .fetch_one(tx.as_mut())
    .await
    .expect("table count query should run in transaction");

    assert_eq!(table_count_in_tx, 4);

    tx.rollback()
        .await
        .expect("rollback should succeed after migrations");

    let table_count_after_rollback: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('checkpoints', 'sessions', 'messages', 'graph_triples')",
    )
    .fetch_one(&pool)
    .await
    .expect("table count query should run after rollback");

    assert_eq!(table_count_after_rollback, 0);
}

#[tokio::test]
async fn ops_sqlite_save_assigns_seq_per_thread() {
    let pool = sqlite_pool().await;

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

    let seqs: Vec<i64> =
        sqlx::query("SELECT seq FROM checkpoints WHERE thread_id = ? ORDER BY seq")
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
async fn ops_sqlite_save_helper_runs_inside_transaction() {
    let pool = sqlite_pool().await;

    run_migrations(&pool)
        .await
        .expect("migrations should bootstrap schema");

    let mut tx = pool
        .begin()
        .await
        .expect("transaction should begin for checkpoint inserts");

    let first = save_checkpoint_in_transaction(
        &mut tx,
        "thread-a",
        "n1",
        1,
        "2026-02-06T00:00:00Z",
        &serde_json::json!({"count": 1}),
    )
    .await
    .expect("first checkpoint should save in transaction");

    let second = save_checkpoint_in_transaction(
        &mut tx,
        "thread-a",
        "n2",
        2,
        "2026-02-06T00:00:01Z",
        &serde_json::json!({"count": 2}),
    )
    .await
    .expect("second checkpoint should save in transaction");

    assert_eq!(first, 1);
    assert_eq!(second, 2);

    let row_count_in_tx: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM checkpoints")
        .fetch_one(tx.as_mut())
        .await
        .expect("count query should run in transaction");

    assert_eq!(row_count_in_tx, 2);

    tx.rollback()
        .await
        .expect("rollback should succeed after checkpoint inserts");

    let row_count_after_rollback: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM checkpoints")
        .fetch_one(&pool)
        .await
        .expect("count query should run after rollback");

    assert_eq!(row_count_after_rollback, 0);
}

#[tokio::test]
async fn ops_sqlite_load_returns_latest_checkpoint_only() {
    let pool = sqlite_pool().await;

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
