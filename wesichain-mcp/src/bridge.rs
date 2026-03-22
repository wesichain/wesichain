//! Bridge between MCP tools and wesichain's `Tool` trait.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use wesichain_agent::tooling::ToolSetBuilder;
use wesichain_core::ToolError;

use serde_json::Value;

use crate::error::McpError;
use crate::protocol::{
    JsonRpcRequest, McpResourceContent, McpResourceSpec, McpToolSpec, SamplingRequest,
    SamplingResult,
};
use crate::stdio::StdioTransport;
use crate::transport::McpTransport;

/// A `wesichain_core::Tool` that dispatches to an MCP server.
pub struct McpTool {
    pub name: String,
    pub description: String,
    schema: Value,
    transport: Arc<dyn McpTransport>,
    next_id: Arc<AtomicI64>,
}

impl McpTool {
    pub fn new(
        spec: McpToolSpec,
        transport: Arc<dyn McpTransport>,
        next_id: Arc<AtomicI64>,
    ) -> Self {
        Self {
            name: spec.name,
            description: spec.description,
            schema: spec.input_schema,
            transport,
            next_id,
        }
    }
}

#[async_trait]
impl wesichain_core::Tool for McpTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn schema(&self) -> Value {
        self.schema.clone()
    }

    async fn invoke(&self, args: Value) -> Result<Value, ToolError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest::new(
            id,
            "tools/call",
            Some(serde_json::json!({
                "name": self.name,
                "arguments": args
            })),
        );

        let resp = self.transport.send(req).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("MCP transport error: {e}"))
        })?;

        if let Some(err) = resp.error {
            return Err(ToolError::ExecutionFailed(format!(
                "MCP RPC error {}: {}",
                err.code, err.message
            )));
        }

        let result = resp.result.ok_or_else(|| {
            ToolError::ExecutionFailed("MCP tools/call returned no result".to_string())
        })?;

        Ok(result)
    }
}

// ── Load helpers ──────────────────────────────────────────────────────────────

/// Load all tools from an MCP server via stdio transport.
///
/// Returns `(Vec<McpTool>, Arc<AtomicI64>)` — the tools share the same atomic
/// ID counter and transport.
pub async fn load_mcp_tools(
    transport: Arc<dyn McpTransport>,
) -> Result<Vec<McpTool>, McpError> {
    let next_id = Arc::new(AtomicI64::new(100));
    let id = next_id.fetch_add(1, Ordering::SeqCst);
    let req = JsonRpcRequest::new(id, "tools/list", None);
    let resp = transport.send(req).await?;

    if let Some(err) = resp.error {
        return Err(McpError::RpcError { code: err.code, message: err.message });
    }

    let result = resp.result.ok_or_else(|| McpError::Protocol("no result".to_string()))?;
    let list: crate::protocol::McpToolsListResult = serde_json::from_value(result)?;

    Ok(list.tools.into_iter().map(|spec| {
        McpTool::new(spec, transport.clone(), next_id.clone())
    }).collect())
}

// ── McpClient ─────────────────────────────────────────────────────────────────

/// High-level MCP client providing tools, resources, and sampling.
///
/// Built on top of any [`McpTransport`]; use [`McpClient::stdio`] for
/// subprocess servers or [`McpClient::http`] for remote servers via HTTP+SSE.
pub struct McpClient {
    transport: Arc<dyn McpTransport>,
    next_id: Arc<AtomicI64>,
}

impl McpClient {
    pub fn new(transport: Arc<dyn McpTransport>) -> Self {
        Self { transport, next_id: Arc::new(AtomicI64::new(1)) }
    }

    /// Create a client backed by a subprocess stdio transport.
    pub async fn stdio(command: &str, args: &[&str]) -> Result<Self, McpError> {
        let transport = Arc::new(crate::stdio::StdioTransport::spawn(command, args).await?);
        Ok(Self::new(transport))
    }

    /// Create a client backed by an HTTP transport pointing at `url`.
    ///
    /// Optionally attach a Bearer token via `.with_bearer_token()` on the
    /// returned transport before wrapping it in `McpClient::new()`.
    pub fn http(url: impl Into<String>) -> Self {
        let transport = Arc::new(crate::http::HttpMcpTransport::new(url));
        Self::new(transport)
    }

    /// Create an HTTP client with Bearer token auth.
    pub fn http_with_token(url: impl Into<String>, token: impl Into<String>) -> Self {
        let transport =
            Arc::new(crate::http::HttpMcpTransport::new(url).with_bearer_token(token));
        Self::new(transport)
    }

    fn next_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    async fn rpc(&self, method: &str, params: Option<Value>) -> Result<Value, McpError> {
        let req = JsonRpcRequest::new(self.next_id(), method, params);
        let resp = self.transport.send(req).await?;
        if let Some(err) = resp.error {
            return Err(McpError::RpcError { code: err.code, message: err.message });
        }
        resp.result.ok_or_else(|| McpError::Protocol(format!("{method} returned no result")))
    }

    // ── Tools ─────────────────────────────────────────────────────────────────

    /// List all tools the server advertises (`tools/list`).
    pub async fn list_tools(&self) -> Result<Vec<McpToolSpec>, McpError> {
        let result = self.rpc("tools/list", None).await?;
        let list: crate::protocol::McpToolsListResult = serde_json::from_value(result)?;
        Ok(list.tools)
    }

    /// Call a named tool (`tools/call`).
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value, McpError> {
        let result = self.rpc(
            "tools/call",
            Some(serde_json::json!({ "name": name, "arguments": args })),
        ).await?;
        Ok(result)
    }

    // ── Resources ─────────────────────────────────────────────────────────────

    /// List all resources the server exposes (`resources/list`).
    pub async fn list_resources(&self) -> Result<Vec<McpResourceSpec>, McpError> {
        let result = self.rpc("resources/list", None).await?;
        let list: crate::protocol::McpResourcesListResult = serde_json::from_value(result)?;
        Ok(list.resources)
    }

    /// Read a resource by URI (`resources/read`).
    ///
    /// Returns the content blocks for the resource.  Text resources carry
    /// `content.text`; binary resources carry `content.blob` (base64).
    pub async fn read_resource(&self, uri: &str) -> Result<Vec<McpResourceContent>, McpError> {
        let result = self.rpc(
            "resources/read",
            Some(serde_json::json!({ "uri": uri })),
        ).await?;
        let read: crate::protocol::McpResourceReadResult = serde_json::from_value(result)?;
        Ok(read.contents)
    }

    // ── Sampling ──────────────────────────────────────────────────────────────

    /// Ask the client's LLM to generate a completion (`sampling/createMessage`).
    ///
    /// This is a server-to-client request in MCP: the MCP server asks the
    /// host application (us) to call the LLM on its behalf.  Here we forward
    /// the request back to the MCP server using the `sampling/createMessage`
    /// JSON-RPC method.
    pub async fn sampling_create_message(
        &self,
        request: SamplingRequest,
    ) -> Result<SamplingResult, McpError> {
        let params = serde_json::to_value(&request)?;
        let result = self.rpc("sampling/createMessage", Some(params)).await?;
        let sampling: SamplingResult = serde_json::from_value(result)?;
        Ok(sampling)
    }

    // ── ToolSet integration ───────────────────────────────────────────────────

    /// Load all tools from this client and return them as `McpTool` instances
    /// ready to register in a `ToolSet`.
    pub async fn as_tools(&self) -> Result<Vec<McpTool>, McpError> {
        let specs = self.list_tools().await?;
        Ok(specs.into_iter().map(|spec| {
            McpTool::new(spec, self.transport.clone(), self.next_id.clone())
        }).collect())
    }
}

// ── ToolSetBuilderMcpExt ──────────────────────────────────────────────────────

/// Extension trait for `ToolSetBuilder` to add MCP servers.
#[async_trait]
pub trait ToolSetBuilderMcpExt: Sized {
    /// Spawn `command args` as an MCP server via stdio and register all its tools.
    async fn add_mcp_server(self, command: &str, args: &[&str]) -> Result<Self, McpError>;
}

#[async_trait]
impl ToolSetBuilderMcpExt for ToolSetBuilder {
    async fn add_mcp_server(mut self, command: &str, args: &[&str]) -> Result<Self, McpError> {
        let transport = Arc::new(StdioTransport::spawn(command, args).await?);
        let tools = load_mcp_tools(transport).await?;
        for tool in tools {
            self = self.register_dynamic(tool);
        }
        Ok(self)
    }
}
