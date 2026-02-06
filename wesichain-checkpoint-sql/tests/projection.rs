use sqlx::Row;
use wesichain_checkpoint_sql::error::CheckpointSqlError;
use wesichain_checkpoint_sql::migrations::run_migrations;
use wesichain_checkpoint_sql::ops::save_checkpoint_with_projections;

async fn sqlite_pool() -> sqlx::SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should connect")
}

fn state_with_session_and_messages() -> serde_json::Value {
    serde_json::json!({
        "session_id": "session-1",
        "messages": [
            {"role": "user", "content": "hello", "created_at": "2026-02-06T00:00:00Z"},
            {"role": "assistant", "content": "hi", "created_at": "2026-02-06T00:00:01Z"}
        ]
    })
}

#[tokio::test]
async fn projection_disabled_writes_only_canonical_checkpoint() {
    let pool = sqlite_pool().await;
    run_migrations(&pool)
        .await
        .expect("migrations should bootstrap schema");

    save_checkpoint_with_projections(
        &pool,
        "thread-a",
        "node-a",
        1,
        "2026-02-06T00:00:00Z",
        &state_with_session_and_messages(),
        false,
    )
    .await
    .expect("checkpoint save should succeed");

    let checkpoints: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM checkpoints")
        .fetch_one(&pool)
        .await
        .expect("checkpoints count query should run");
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&pool)
        .await
        .expect("sessions count query should run");
    let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
        .fetch_one(&pool)
        .await
        .expect("messages count query should run");

    assert_eq!(checkpoints, 1);
    assert_eq!(sessions, 0);
    assert_eq!(messages, 0);
}

#[tokio::test]
async fn projection_enabled_writes_sessions_and_messages() {
    let pool = sqlite_pool().await;
    run_migrations(&pool)
        .await
        .expect("migrations should bootstrap schema");

    save_checkpoint_with_projections(
        &pool,
        "thread-a",
        "node-a",
        1,
        "2026-02-06T00:00:00Z",
        &state_with_session_and_messages(),
        true,
    )
    .await
    .expect("checkpoint save with projections should succeed");

    let checkpoints: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM checkpoints")
        .fetch_one(&pool)
        .await
        .expect("checkpoints count query should run");
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&pool)
        .await
        .expect("sessions count query should run");
    let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
        .fetch_one(&pool)
        .await
        .expect("messages count query should run");

    assert_eq!(checkpoints, 1);
    assert_eq!(sessions, 1);
    assert_eq!(messages, 2);

    let session = sqlx::query("SELECT session_id FROM sessions WHERE thread_id = ?")
        .bind("thread-a")
        .fetch_one(&pool)
        .await
        .expect("session row should exist");
    let session_id: Option<String> = session.get("session_id");
    assert_eq!(session_id.as_deref(), Some("session-1"));
}

#[tokio::test]
async fn projection_error_rolls_back_checkpoint_insert() {
    let pool = sqlite_pool().await;
    run_migrations(&pool)
        .await
        .expect("migrations should bootstrap schema");

    let bad_state = serde_json::json!({
        "session_id": "session-1",
        "messages": [
            {"role": "user"}
        ]
    });

    let error = save_checkpoint_with_projections(
        &pool,
        "thread-a",
        "node-a",
        1,
        "2026-02-06T00:00:00Z",
        &bad_state,
        true,
    )
    .await
    .expect_err("projection mapping should fail");

    assert!(matches!(error, CheckpointSqlError::Projection(_)));

    let checkpoints: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM checkpoints")
        .fetch_one(&pool)
        .await
        .expect("checkpoints count query should run");
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&pool)
        .await
        .expect("sessions count query should run");
    let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
        .fetch_one(&pool)
        .await
        .expect("messages count query should run");

    assert_eq!(checkpoints, 0);
    assert_eq!(sessions, 0);
    assert_eq!(messages, 0);
}
