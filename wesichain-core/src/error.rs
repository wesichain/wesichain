use std::{error::Error as StdError, fmt, time::Duration};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WesichainError {
    #[error("LLM provider failed: {0}")]
    LlmProvider(String),
    #[error("Tool call failed for '{tool_name}': {reason}")]
    ToolCallFailed { tool_name: String, reason: String },
    #[error("Parsing failed on output '{output}': {reason}")]
    ParseFailed { output: String, reason: String },
    #[error("Operation timed out after {0:?}")]
    Timeout(Duration),
    #[error("Max retries ({max}) exceeded")]
    MaxRetriesExceeded { max: usize },
    #[error("Checkpoint failed: {0}")]
    CheckpointFailed(String),
    #[error("Operation was cancelled")]
    Cancelled,
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Serialization/deserialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("{0}")]
    Custom(String),
}

#[derive(Debug)]
pub enum EmbeddingError {
    InvalidResponse(String),
    RateLimited { retry_after: Option<Duration> },
    Timeout(Duration),
    Provider(String),
    Other(Box<dyn StdError + Send + Sync>),
}

impl fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmbeddingError::InvalidResponse(message) => {
                write!(f, "Invalid embedding response: {message}")
            }
            EmbeddingError::RateLimited { retry_after } => match retry_after {
                Some(duration) => write!(f, "embedding rate limited (retry_after={duration:?})"),
                None => write!(f, "embedding rate limited (retry_after=unknown)"),
            },
            EmbeddingError::Timeout(duration) => write!(f, "embedding timeout after {duration:?}"),
            EmbeddingError::Provider(message) => write!(f, "Embedding provider failed: {message}"),
            EmbeddingError::Other(error) => write!(f, "Embedding error: {error}"),
        }
    }
}

impl StdError for EmbeddingError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            EmbeddingError::Other(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
