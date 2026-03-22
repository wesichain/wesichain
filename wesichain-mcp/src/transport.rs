//! MCP transport abstraction.

use async_trait::async_trait;

use crate::error::McpError;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};

/// Low-level JSON-RPC transport used by the MCP client.
#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn send(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, McpError>;
}
