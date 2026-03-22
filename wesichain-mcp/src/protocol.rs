//! JSON-RPC 2.0 types + MCP message types (spec 2024-11-05).

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── JSON-RPC 2.0 ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: JsonRpcId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcId {
    Number(i64),
    String(String),
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: JsonRpcId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: i64, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(id),
            method: method.into(),
            params,
        }
    }
}

// ── MCP types — tools ─────────────────────────────────────────────────────────

/// A tool advertised by an MCP server.
#[derive(Debug, Clone, Deserialize)]
pub struct McpToolSpec {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Wrapper around the MCP `tools/list` response.
#[derive(Debug, Deserialize)]
pub struct McpToolsListResult {
    pub tools: Vec<McpToolSpec>,
}

/// Wrapper around the MCP `tools/call` response.
#[derive(Debug, Deserialize)]
pub struct McpToolCallResult {
    pub content: Vec<McpContent>,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Deserialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(default)]
    pub text: String,
}

// ── MCP types — resources ─────────────────────────────────────────────────────

/// A resource advertised by an MCP server (`resources/list`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceSpec {
    /// Stable URI identifying the resource (e.g. `file:///src/main.rs`).
    pub uri: String,
    /// Human-readable name.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type hint (e.g. `text/x-rust`).
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Wrapper around the `resources/list` response.
#[derive(Debug, Deserialize)]
pub struct McpResourcesListResult {
    pub resources: Vec<McpResourceSpec>,
}

/// A single piece of content returned by `resources/read`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceContent {
    pub uri: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// UTF-8 text content (for text resources).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Base64-encoded binary content (for blob resources).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

/// Wrapper around the `resources/read` response.
#[derive(Debug, Deserialize)]
pub struct McpResourceReadResult {
    pub contents: Vec<McpResourceContent>,
}

// ── MCP types — sampling ──────────────────────────────────────────────────────

/// A message in a `sampling/createMessage` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingMessage {
    pub role: String,
    pub content: SamplingContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SamplingContent {
    Text { text: String },
}

impl SamplingMessage {
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: "user".to_string(), content: SamplingContent::Text { text: text.into() } }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self { role: "assistant".to_string(), content: SamplingContent::Text { text: text.into() } }
    }
}

/// Parameters for `sampling/createMessage`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingRequest {
    pub messages: Vec<SamplingMessage>,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// Response from `sampling/createMessage`.
#[derive(Debug, Clone, Deserialize)]
pub struct SamplingResult {
    pub role: String,
    pub content: SamplingContent,
    pub model: String,
    #[serde(rename = "stopReason", skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}
