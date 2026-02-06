use thiserror::Error;

#[derive(Debug, Error)]
pub enum CheckpointSqlError {
    #[error("checkpoint SQL connection error: {0}")]
    Connection(String),
    #[error("checkpoint SQL migration error: {0}")]
    Migration(String),
    #[error("checkpoint SQL serialization error: {0}")]
    Serialization(String),
    #[error("checkpoint SQL query error: {0}")]
    Query(String),
    #[error("checkpoint SQL projection error: {0}")]
    Projection(String),
    #[error("SQL checkpoint operation is not implemented")]
    NotImplemented,
}
