//! MCP error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("JSON-RPC error {code}: {message}")]
    RpcError { code: i32, message: String },
    #[error("Transport closed")]
    Closed,
    #[error("MCP protocol error: {0}")]
    Protocol(String),
    #[error("Transport error: {0}")]
    Transport(String),
}
