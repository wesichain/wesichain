use std::time::Duration;

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

#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("Embedding provider failed: {0}")]
    Provider(String),
    #[error("Invalid embedding input: {0}")]
    InvalidInput(String),
    #[error("{0}")]
    Custom(String),
}
