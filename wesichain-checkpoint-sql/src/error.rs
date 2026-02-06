use thiserror::Error;

#[derive(Debug, Error)]
pub enum CheckpointSqlError {
    #[error("SQL checkpoint operation is not implemented")]
    NotImplemented,
}
