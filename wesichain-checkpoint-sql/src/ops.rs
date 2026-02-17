use crate::error::CheckpointSqlError;
use crate::projection::{apply_projection_rows_in_transaction, map_state_to_projection_rows};
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;
use sqlx::{ColumnIndex, Database, Pool, QueryBuilder};

const SAVE_RETRY_LIMIT: usize = 8;

fn should_retry_sequence_write(error: &sqlx::Error) -> bool {
    if let Some(db_error) = error.as_database_error() {
        if db_error.is_unique_violation() {
            return true;
        }

        let message = db_error.message().to_ascii_lowercase();
        return message.contains("database is locked")
            || message.contains("deadlock")
            || message.contains("serialization");
    }

    false
}

pub async fn next_checkpoint_seq_in_transaction<DB>(
    tx: &mut sqlx::Transaction<'_, DB>,
    thread_id: &str,
) -> Result<i64, CheckpointSqlError>
where
    DB: Database,
    for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
    usize: ColumnIndex<DB::Row>,
{
    let seq_sql = {
        let mut query = QueryBuilder::<DB>::new(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM checkpoints WHERE thread_id = ",
        );
        query.push_bind(thread_id);
        query.sql().to_owned()
    };

    sqlx::query_scalar::<DB, i64>(&seq_sql)
        .bind(thread_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(CheckpointSqlError::Query)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_checkpoint_in_transaction<DB>(
    tx: &mut sqlx::Transaction<'_, DB>,
    thread_id: &str,
    seq: i64,
    created_at: &str,
    node: &str,
    step: i64,
    state_json: &str,
    queue_json: &str,
) -> Result<(), CheckpointSqlError>
where
    DB: Database,
    for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
{
    let insert_sql = {
        let mut query = QueryBuilder::<DB>::new(
            "INSERT INTO checkpoints (thread_id, seq, created_at, node, step, state_json, queue_json) VALUES (",
        );
        query
            .push_bind(thread_id)
            .push(", ")
            .push_bind(seq)
            .push(", ")
            .push_bind(created_at)
            .push(", ")
            .push_bind(node)
            .push(", ")
            .push_bind(step)
            .push(", ")
            .push_bind(state_json)
            .push(", ")
            .push_bind(queue_json)
            .push(")");
        query.sql().to_owned()
    };

    sqlx::query::<DB>(&insert_sql)
        .bind(thread_id)
        .bind(seq)
        .bind(created_at)
        .bind(node)
        .bind(step)
        .bind(state_json)
        .bind(queue_json)
        .execute(tx.as_mut())
        .await
        .map_err(CheckpointSqlError::Query)?;

    Ok(())
}

pub async fn save_checkpoint_in_transaction_with_queue<DB, S, Q>(
    tx: &mut sqlx::Transaction<'_, DB>,
    thread_id: &str,
    node: &str,
    step: i64,
    created_at: &str,
    state: &S,
    queue: &Q,
) -> Result<i64, CheckpointSqlError>
where
    DB: Database,
    for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
    usize: ColumnIndex<DB::Row>,
    S: Serialize + ?Sized,
    Q: Serialize + ?Sized,
{
    let state_json = serde_json::to_string(state).map_err(CheckpointSqlError::Serialization)?;
    let queue_json = serde_json::to_string(queue).map_err(CheckpointSqlError::Serialization)?;
    let seq = next_checkpoint_seq_in_transaction(tx, thread_id).await?;

    insert_checkpoint_in_transaction(
        tx,
        thread_id,
        seq,
        created_at,
        node,
        step,
        &state_json,
        &queue_json,
    )
    .await?;

    Ok(seq)
}

pub async fn save_checkpoint_in_transaction<DB, S>(
    tx: &mut sqlx::Transaction<'_, DB>,
    thread_id: &str,
    node: &str,
    step: i64,
    created_at: &str,
    state: &S,
) -> Result<i64, CheckpointSqlError>
where
    DB: Database,
    for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
    usize: ColumnIndex<DB::Row>,
    S: Serialize + ?Sized,
{
    save_checkpoint_in_transaction_with_queue(
        tx,
        thread_id,
        node,
        step,
        created_at,
        state,
        &Vec::<(String, u64)>::new(),
    )
    .await
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredCheckpoint {
    pub thread_id: String,
    pub seq: i64,
    pub created_at: String,
    pub node: Option<String>,
    pub step: Option<i64>,
    pub state_json: Value,
    pub queue_json: Value,
}

pub async fn save_checkpoint_with_queue<DB, S, Q>(
    pool: &Pool<DB>,
    thread_id: &str,
    node: &str,
    step: i64,
    created_at: &str,
    state: &S,
    queue: &Q,
) -> Result<i64, CheckpointSqlError>
where
    DB: Database,
    for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> String: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c Pool<DB>: sqlx::Executor<'c, Database = DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
    for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    usize: ColumnIndex<DB::Row>,
    S: Serialize + ?Sized,
    Q: Serialize + ?Sized,
{
    let mut last_error: Option<CheckpointSqlError> = None;

    for _attempt in 0..SAVE_RETRY_LIMIT {
        let mut tx = pool.begin().await.map_err(CheckpointSqlError::Query)?;
        match save_checkpoint_in_transaction_with_queue(
            &mut tx, thread_id, node, step, created_at, state, queue,
        )
        .await
        {
            Ok(seq) => {
                if let Err(commit_error) = tx.commit().await {
                    if should_retry_sequence_write(&commit_error) {
                        last_error = Some(CheckpointSqlError::Query(commit_error));
                        continue;
                    }
                    return Err(CheckpointSqlError::Query(commit_error));
                }
                return Ok(seq);
            }
            Err(CheckpointSqlError::Query(query_error)) => {
                let should_retry = should_retry_sequence_write(&query_error);
                let _ = tx.rollback().await;
                if should_retry {
                    last_error = Some(CheckpointSqlError::Query(query_error));
                    continue;
                }
                return Err(CheckpointSqlError::Query(query_error));
            }
            Err(other) => {
                let _ = tx.rollback().await;
                return Err(other);
            }
        }
    }

    Err(last_error.unwrap_or(CheckpointSqlError::NotImplemented))
}

pub async fn save_checkpoint<DB, S>(
    pool: &Pool<DB>,
    thread_id: &str,
    node: &str,
    step: i64,
    created_at: &str,
    state: &S,
) -> Result<i64, CheckpointSqlError>
where
    DB: Database,
    for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> String: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c Pool<DB>: sqlx::Executor<'c, Database = DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
    for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    usize: ColumnIndex<DB::Row>,
    S: Serialize + ?Sized,
{
    save_checkpoint_with_queue(
        pool,
        thread_id,
        node,
        step,
        created_at,
        state,
        &Vec::<(String, u64)>::new(),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn save_checkpoint_with_projections_and_queue<DB, S, Q>(
    pool: &Pool<DB>,
    thread_id: &str,
    node: &str,
    step: i64,
    created_at: &str,
    state: &S,
    queue: &Q,
    enable_projections: bool,
) -> Result<i64, CheckpointSqlError>
where
    DB: Database,
    for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> String: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> Option<String>: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c Pool<DB>: sqlx::Executor<'c, Database = DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
    for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    usize: ColumnIndex<DB::Row>,
    S: Serialize + ?Sized,
    Q: Serialize + ?Sized,
{
    let mut last_error: Option<CheckpointSqlError> = None;

    for _attempt in 0..SAVE_RETRY_LIMIT {
        let mut tx = pool.begin().await.map_err(CheckpointSqlError::Query)?;
        match save_checkpoint_in_transaction_with_queue(
            &mut tx, thread_id, node, step, created_at, state, queue,
        )
        .await
        {
            Ok(seq) => {
                if enable_projections {
                    let state_json =
                        serde_json::to_value(state).map_err(CheckpointSqlError::Serialization)?;
                    let projection_rows =
                        map_state_to_projection_rows(&state_json, seq, created_at)?;
                    if let Err(error) =
                        apply_projection_rows_in_transaction(&mut tx, thread_id, &projection_rows)
                            .await
                    {
                        let _ = tx.rollback().await;
                        return Err(error);
                    }
                }

                if let Err(commit_error) = tx.commit().await {
                    if should_retry_sequence_write(&commit_error) {
                        last_error = Some(CheckpointSqlError::Query(commit_error));
                        continue;
                    }
                    return Err(CheckpointSqlError::Query(commit_error));
                }
                return Ok(seq);
            }
            Err(CheckpointSqlError::Query(query_error)) => {
                let should_retry = should_retry_sequence_write(&query_error);
                let _ = tx.rollback().await;
                if should_retry {
                    last_error = Some(CheckpointSqlError::Query(query_error));
                    continue;
                }
                return Err(CheckpointSqlError::Query(query_error));
            }
            Err(other) => {
                let _ = tx.rollback().await;
                return Err(other);
            }
        }
    }

    Err(last_error.unwrap_or(CheckpointSqlError::NotImplemented))
}

pub async fn save_checkpoint_with_projections<DB, S>(
    pool: &Pool<DB>,
    thread_id: &str,
    node: &str,
    step: i64,
    created_at: &str,
    state: &S,
    enable_projections: bool,
) -> Result<i64, CheckpointSqlError>
where
    DB: Database,
    for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> String: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> Option<String>: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c Pool<DB>: sqlx::Executor<'c, Database = DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
    for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    usize: ColumnIndex<DB::Row>,
    S: Serialize + ?Sized,
{
    save_checkpoint_with_projections_and_queue(
        pool,
        thread_id,
        node,
        step,
        created_at,
        state,
        &Vec::<(String, u64)>::new(),
        enable_projections,
    )
    .await
}

pub async fn load_latest_checkpoint<DB>(
    pool: &Pool<DB>,
    thread_id: &str,
) -> Result<Option<StoredCheckpoint>, CheckpointSqlError>
where
    DB: Database,
    for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c Pool<DB>: sqlx::Executor<'c, Database = DB>,
    &'static str: ColumnIndex<DB::Row>,
    for<'r> String: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    for<'r> Option<String>: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    for<'r> Option<i64>: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
{
    let select_sql = {
        let mut query = QueryBuilder::<DB>::new(
            "SELECT thread_id, seq, created_at, node, step, state_json, queue_json FROM checkpoints WHERE thread_id = ",
        );
        query
            .push_bind(thread_id)
            .push(" ORDER BY seq DESC LIMIT 1");
        query.sql().to_owned()
    };

    let row = sqlx::query::<DB>(&select_sql)
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
    let queue_json_str: String = row.get("queue_json");
    let queue_json: Value =
        serde_json::from_str(&queue_json_str).map_err(CheckpointSqlError::Serialization)?;

    Ok(Some(StoredCheckpoint {
        thread_id: row.get("thread_id"),
        seq: row.get("seq"),
        created_at: row.get("created_at"),
        node: row.get("node"),
        step: row.get("step"),
        state_json,
        queue_json,
    }))
}

pub async fn load_checkpoint<DB>(
    pool: &Pool<DB>,
    thread_id: &str,
) -> Result<Option<Value>, CheckpointSqlError>
where
    DB: Database,
    for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c Pool<DB>: sqlx::Executor<'c, Database = DB>,
    &'static str: ColumnIndex<DB::Row>,
    for<'r> String: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    for<'r> Option<String>: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    for<'r> Option<i64>: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
{
    Ok(load_latest_checkpoint(pool, thread_id)
        .await?
        .map(|checkpoint| checkpoint.state_json))
}
