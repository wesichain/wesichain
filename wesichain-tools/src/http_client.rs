//! HTTP tools: GET and POST over the network.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wesichain_core::{ToolContext, ToolError, TypedTool};

// ── HttpGetTool ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct HttpGetArgs {
    /// URL to fetch
    pub url: String,
    /// Optional extra headers (key → value)
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct HttpGetOutput {
    pub status: u16,
    pub body: Value,
}

#[derive(Clone, Default)]
pub struct HttpGetTool;

#[async_trait::async_trait]
impl TypedTool for HttpGetTool {
    type Args = HttpGetArgs;
    type Output = HttpGetOutput;
    const NAME: &'static str = "http_get";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        let client = reqwest::Client::new();
        let mut req = client.get(&args.url);

        if let Some(headers) = args.headers {
            for (k, v) in headers {
                req = req.header(k, v);
            }
        }

        let resp = req.send().await.map_err(|e| {
            ToolError::ExecutionFailed(format!("HTTP GET failed: {e}"))
        })?;

        let status = resp.status().as_u16();
        let text = resp.text().await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read response body: {e}"))
        })?;

        let body: Value = serde_json::from_str(&text)
            .unwrap_or_else(|_| Value::Object({
                let mut m = serde_json::Map::new();
                m.insert("text".to_string(), Value::String(text));
                m
            }));

        Ok(HttpGetOutput { status, body })
    }
}

// ── HttpPostTool ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct HttpPostArgs {
    /// URL to POST to
    pub url: String,
    /// JSON body to send
    pub body: Value,
    /// Optional extra headers (key → value)
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct HttpPostOutput {
    pub status: u16,
    pub body: Value,
}

#[derive(Clone, Default)]
pub struct HttpPostTool;

#[async_trait::async_trait]
impl TypedTool for HttpPostTool {
    type Args = HttpPostArgs;
    type Output = HttpPostOutput;
    const NAME: &'static str = "http_post";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        let client = reqwest::Client::new();
        let mut req = client.post(&args.url).json(&args.body);

        if let Some(headers) = args.headers {
            for (k, v) in headers {
                req = req.header(k, v);
            }
        }

        let resp = req.send().await.map_err(|e| {
            ToolError::ExecutionFailed(format!("HTTP POST failed: {e}"))
        })?;

        let status = resp.status().as_u16();
        let text = resp.text().await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read response body: {e}"))
        })?;

        let body: Value = serde_json::from_str(&text)
            .unwrap_or_else(|_| Value::Object({
                let mut m = serde_json::Map::new();
                m.insert("text".to_string(), Value::String(text));
                m
            }));

        Ok(HttpPostOutput { status, body })
    }
}
