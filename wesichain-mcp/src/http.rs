//! HTTP+SSE transport for remote MCP servers.
//!
//! Implements [`McpTransport`] over HTTP, using `POST` requests carrying
//! JSON-RPC 2.0 payloads.  Remote MCP servers (GitHub Copilot, databases,
//! SaaS integrations) expose this interface.
//!
//! # Example
//! ```ignore
//! use wesichain_mcp::{HttpMcpTransport, McpClient};
//!
//! let transport = HttpMcpTransport::new("https://mcp.example.com/mcp")
//!     .with_bearer_token("my-token");
//! let client = McpClient::new(std::sync::Arc::new(transport));
//! let resources = client.list_resources().await?;
//! ```

use async_trait::async_trait;
use reqwest::Client;

use crate::error::McpError;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::transport::McpTransport;

/// MCP transport that speaks JSON-RPC over HTTP POST.
///
/// Compatible with remote MCP servers that accept `application/json` bodies
/// and return JSON-RPC responses.  Bearer token auth is optional.
pub struct HttpMcpTransport {
    url: String,
    client: Client,
    bearer_token: Option<String>,
}

impl HttpMcpTransport {
    /// Create a new transport pointed at `url`.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("valid reqwest client"),
            bearer_token: None,
        }
    }

    /// Attach a Bearer token to every request.
    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }
}

#[async_trait]
impl McpTransport for HttpMcpTransport {
    async fn send(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, McpError> {
        let mut builder = self
            .client
            .post(&self.url)
            .header("content-type", "application/json")
            .json(&request);

        if let Some(token) = &self.bearer_token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(McpError::Transport(format!("HTTP {status}: {body}")));
        }

        resp.json::<JsonRpcResponse>()
            .await
            .map_err(|e| McpError::Protocol(format!("JSON-RPC parse error: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_with_url_and_token() {
        let t = HttpMcpTransport::new("https://mcp.example.com").with_bearer_token("tok");
        assert_eq!(t.url, "https://mcp.example.com");
        assert_eq!(t.bearer_token.as_deref(), Some("tok"));
    }
}
