use crate::error::CheckpointSqlError;
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;

#[derive(Debug, Clone, PartialEq)]
pub struct StoredCheckpoint {
    pub thread_id: String,
    pub seq: i64,
    pub created_at: String,
    pub node: Option<String>,
    pub step: Option<i64>,
    pub state_json: Value,
}

pub async fn save_checkpoint<S>(
    pool: &sqlx::SqlitePool,
    thread_id: &str,
    node: &str,
    step: i64,
    created_at: &str,
    state: &S,
) -> Result<i64, CheckpointSqlError>
where
    S: Serialize + ?Sized,
{
    let state_json = serde_json::to_string(state).map_err(CheckpointSqlError::Serialization)?;
    let mut tx = pool
        .begin()
        .await
        .map_err(CheckpointSqlError::Connection)?;

    let seq: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(seq), 0) + 1 FROM checkpoints WHERE thread_id = ?",
    )
    .bind(thread_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(CheckpointSqlError::Query)?;

    sqlx::query(
        "INSERT INTO checkpoints (thread_id, seq, created_at, node, step, state_json) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(thread_id)
    .bind(seq)
    .bind(created_at)
    .bind(node)
    .bind(step)
    .bind(state_json)
    .execute(&mut *tx)
    .await
    .map_err(CheckpointSqlError::Query)?;

    tx.commit().await.map_err(CheckpointSqlError::Query)?;

    Ok(seq)
}

pub async fn load_latest_checkpoint(
    pool: &sqlx::SqlitePool,
    thread_id: &str,
) -> Result<Option<StoredCheckpoint>, CheckpointSqlError> {
    let row = sqlx::query(
        "SELECT thread_id, seq, created_at, node, step, state_json FROM checkpoints WHERE thread_id = ? ORDER BY seq DESC LIMIT 1",
    )
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map_err(CheckpointSqlError::Query)?;

    let Some(row) = row else {
        return Ok(None);
    };

    let state_json_str: String = row.get("state_json");
    let state_json: Value =
        serde_json::from_str(&state_json_str).map_err(CheckpointSqlError::Serialization)?;

    Ok(Some(StoredCheckpoint {
        thread_id: row.get("thread_id"),
        seq: row.get("seq"),
        created_at: row.get("created_at"),
        node: row.get("node"),
        step: row.get("step"),
        state_json,
    }))
}

pub async fn load_checkpoint(
    pool: &sqlx::SqlitePool,
    thread_id: &str,
) -> Result<Option<Value>, CheckpointSqlError> {
    Ok(load_latest_checkpoint(pool, thread_id)
        .await?
        .map(|checkpoint| checkpoint.state_json))
}
