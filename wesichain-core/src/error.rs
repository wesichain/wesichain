use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WesichainError {
    #[error("LLM provider error: {0}")]
    LlmProvider(String),
    #[error("Tool call failed ({tool_name}): {reason}")]
    ToolCallFailed { tool_name: String, reason: String },
    #[error("Parse failed: {reason}. Output: {output}")]
    ParseFailed { output: String, reason: String },
    #[error("Timeout after {0:?}")]
    Timeout(Duration),
    #[error("Max retries ({max}) exceeded")]
    MaxRetriesExceeded { max: usize },
    #[error("Checkpoint failed: {0}")]
    CheckpointFailed(String),
    #[error("Operation cancelled")]
    Cancelled,
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Custom error: {0}")]
    Custom(String),
}
