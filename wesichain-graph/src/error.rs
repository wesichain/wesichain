use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("checkpoint failed: {0}")]
    Checkpoint(String),
}
