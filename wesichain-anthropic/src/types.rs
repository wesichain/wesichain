//! Anthropic Messages API request and response types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// Configuration for Anthropic extended thinking (Claude 3.7+ Sonnet).
///
/// Pass this via [`AnthropicRequest::thinking`].  Requires the
/// `interleaved-thinking-2025-01-05` beta header on the request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub type_: String,
    /// Maximum tokens the model may spend thinking before responding.
    pub budget_tokens: u32,
}

impl ThinkingConfig {
    pub fn new(budget_tokens: u32) -> Self {
        Self { type_: "enabled".to_string(), budget_tokens }
    }
}

#[derive(Debug, Serialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Extended thinking configuration. When set, the `interleaved-thinking-2025-01-05`
    /// beta header is required on the request (added by [`AnthropicClient::with_thinking`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
}

#[derive(Debug, Serialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: AnthropicContent,
}

/// Anthropic message content: either a plain string or an array of parts.
///
/// Serializes `Text(s)` as a JSON string and `Parts(vec)` as a JSON array,
/// matching the Anthropic Messages API wire format.
#[derive(Debug)]
pub enum AnthropicContent {
    Text(String),
    Parts(Vec<AnthropicPart>),
}

impl Serialize for AnthropicContent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            AnthropicContent::Text(s) => serializer.serialize_str(s),
            AnthropicContent::Parts(parts) => parts.serialize(serializer),
        }
    }
}

/// A single typed part within an Anthropic message.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicPart {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
    #[serde(rename = "image")]
    Image { source: AnthropicImageSource },
}

#[derive(Debug, Serialize, Clone)]
pub struct AnthropicImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AnthropicTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AnthropicResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub role: String,
    pub content: Vec<ResponseContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: AnthropicUsage,
}

/// A content block in an Anthropic non-streaming response.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: Value },
    /// Extended thinking block — present when `thinking` is enabled.
    Thinking { thinking: String },
}

#[derive(Debug, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// ---------------------------------------------------------------------------
// Error response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AnthropicErrorResponse {
    #[serde(rename = "type")]
    pub type_: String,
    pub error: AnthropicErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicErrorDetail {
    #[serde(rename = "type")]
    pub type_: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// SSE / streaming event types
// ---------------------------------------------------------------------------

/// Parsed SSE event from the Anthropic streaming API.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    MessageStart {
        message: Value,
    },
    ContentBlockStart {
        index: u32,
        content_block: ContentBlockType,
    },
    ContentBlockDelta {
        index: u32,
        delta: ContentDelta,
    },
    ContentBlockStop {
        index: u32,
    },
    MessageDelta {
        delta: MessageDeltaData,
        #[serde(default)]
        usage: Option<AnthropicDeltaUsage>,
    },
    MessageStop,
    Ping,
    #[serde(other)]
    Other,
}

/// The type of a content block as seen in `content_block_start`.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockType {
    Text { text: String },
    ToolUse { id: String, name: String },
    Thinking { thinking: String },
}

/// Delta payload for `content_block_delta`.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    /// Streaming thinking text delta — emitted when extended thinking is enabled.
    ThinkingDelta { thinking: String },
}

#[derive(Debug, Deserialize)]
pub struct MessageDeltaData {
    pub stop_reason: Option<String>,
    pub usage: Option<AnthropicDeltaUsage>,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicDeltaUsage {
    pub output_tokens: u32,
}
