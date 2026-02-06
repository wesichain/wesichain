use thiserror::Error;

#[derive(Debug, Error)]
pub enum CheckpointSqlError {
    #[error("checkpoint SQL connection error: {0}")]
    Connection(#[source] sqlx::Error),
    #[error("checkpoint SQL migration error: {0}")]
    Migration(#[source] sqlx::Error),
    #[error("checkpoint SQL serialization error: {0}")]
    Serialization(#[source] serde_json::Error),
    #[error("checkpoint SQL query error: {0}")]
    Query(#[source] sqlx::Error),
    #[error("checkpoint SQL projection error: {0}")]
    Projection(String),
    #[error("SQL checkpoint operation is not implemented")]
    NotImplemented,
}

impl From<sqlx::Error> for CheckpointSqlError {
    fn from(source: sqlx::Error) -> Self {
        Self::Query(source)
    }
}

impl From<serde_json::Error> for CheckpointSqlError {
    fn from(source: serde_json::Error) -> Self {
        Self::Serialization(source)
    }
}
