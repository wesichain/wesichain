use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("session not found: {0}")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("session store error: {0}")]
    Custom(String),
}
