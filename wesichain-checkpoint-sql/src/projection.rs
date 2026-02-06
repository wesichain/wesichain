use crate::error::CheckpointSqlError;
use serde_json::Value;
use sqlx::{Database, QueryBuilder};

#[derive(Debug, Clone, PartialEq)]
pub struct SessionProjectionRow {
    pub session_id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageProjectionRow {
    pub seq: i64,
    pub role: String,
    pub content: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProjectionRows {
    pub session: Option<SessionProjectionRow>,
    pub messages: Vec<MessageProjectionRow>,
}

fn data_root(state_json: &Value) -> &Value {
    state_json.get("data").unwrap_or(state_json)
}

fn opt_string(value: Option<&Value>, field: &str) -> Result<Option<String>, CheckpointSqlError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(CheckpointSqlError::Projection(format!(
            "{field} must be a string when present"
        ))),
    }
}

pub fn map_state_to_projection_rows(
    state_json: &Value,
    checkpoint_seq: i64,
    default_created_at: &str,
) -> Result<ProjectionRows, CheckpointSqlError> {
    let mut rows = ProjectionRows::default();
    let root = data_root(state_json);

    let session_id = opt_string(root.get("session_id"), "session_id")?;
    let session_created_at = opt_string(root.get("created_at"), "created_at")?;
    let session_updated_at = opt_string(root.get("updated_at"), "updated_at")?;

    if session_id.is_some() || session_created_at.is_some() || session_updated_at.is_some() {
        rows.session = Some(SessionProjectionRow {
            session_id,
            created_at: session_created_at,
            updated_at: session_updated_at,
        });
    }

    if let Some(messages) = root.get("messages") {
        let messages = messages.as_array().ok_or_else(|| {
            CheckpointSqlError::Projection("messages must be an array when present".to_string())
        })?;

        for (index, message) in messages.iter().enumerate() {
            let role = message
                .get("role")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    CheckpointSqlError::Projection(
                        "each message must have a string role".to_string(),
                    )
                })?
                .to_string();
            let content = message
                .get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    CheckpointSqlError::Projection(
                        "each message must have a string content".to_string(),
                    )
                })?
                .to_string();
            let created_at = opt_string(message.get("created_at"), "message.created_at")?;

            rows.messages.push(MessageProjectionRow {
                seq: checkpoint_seq
                    .saturating_mul(1_000_000)
                    .saturating_add((index as i64) + 1),
                role,
                content,
                created_at: created_at.or_else(|| Some(default_created_at.to_string())),
            });
        }
    }

    Ok(rows)
}

pub async fn apply_projection_rows_in_transaction<DB>(
    tx: &mut sqlx::Transaction<'_, DB>,
    thread_id: &str,
    rows: &ProjectionRows,
) -> Result<(), CheckpointSqlError>
where
    DB: Database,
    for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> Option<String>: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> String: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
{
    if let Some(session) = &rows.session {
        let upsert_sql = {
            let mut query = QueryBuilder::<DB>::new(
                "INSERT INTO sessions (thread_id, session_id, created_at, updated_at) VALUES (",
            );
            query
                .push_bind(thread_id)
                .push(", ")
                .push_bind(session.session_id.clone())
                .push(", ")
                .push_bind(session.created_at.clone())
                .push(", ")
                .push_bind(session.updated_at.clone())
                .push(") ON CONFLICT(thread_id) DO UPDATE SET session_id = excluded.session_id, created_at = excluded.created_at, updated_at = excluded.updated_at");
            query.sql().to_owned()
        };

        sqlx::query::<DB>(&upsert_sql)
            .bind(thread_id)
            .bind(session.session_id.clone())
            .bind(session.created_at.clone())
            .bind(session.updated_at.clone())
            .execute(tx.as_mut())
            .await
            .map_err(CheckpointSqlError::Query)?;
    }

    for message in &rows.messages {
        let insert_sql = {
            let mut query = QueryBuilder::<DB>::new(
                "INSERT INTO messages (thread_id, seq, role, content, created_at) VALUES (",
            );
            query
                .push_bind(thread_id)
                .push(", ")
                .push_bind(message.seq)
                .push(", ")
                .push_bind(message.role.as_str())
                .push(", ")
                .push_bind(message.content.as_str())
                .push(", ")
                .push_bind(message.created_at.clone())
                .push(")");
            query.sql().to_owned()
        };

        sqlx::query::<DB>(&insert_sql)
            .bind(thread_id)
            .bind(message.seq)
            .bind(message.role.as_str())
            .bind(message.content.as_str())
            .bind(message.created_at.clone())
            .execute(tx.as_mut())
            .await
            .map_err(CheckpointSqlError::Query)?;
    }

    Ok(())
}
