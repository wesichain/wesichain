//! Anthropic Claude LLM client.
//!
//! Implements [`Runnable<LlmRequest, LlmResponse>`] for the Anthropic Messages
//! API (<https://docs.anthropic.com/en/api/messages>).

use std::time::Duration;

use futures::stream::{self, BoxStream, StreamExt};
use reqwest::Client;
use wesichain_core::{
    LlmRequest, LlmResponse, Message, MessageContent, Role, Runnable, StreamEvent, TokenUsage,
    ToolCall, ToolSpec, WesichainError,
};

use crate::{
    stream::parse_anthropic_stream,
    types::{
        AnthropicContent, AnthropicErrorResponse, AnthropicMessage, AnthropicPart,
        AnthropicRequest, AnthropicTool, ResponseContentBlock, ThinkingConfig,
    },
};

const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";
pub const MAX_TOKENS_DEFAULT: u32 = 4096;

// ---------------------------------------------------------------------------
// Client struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AnthropicClient {
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) base_url: String,
    pub(crate) http: Client,
    /// When set, extended thinking is enabled on every request.
    pub(crate) thinking: Option<ThinkingConfig>,
}

impl AnthropicClient {
    /// Create a new client with the given API key and default model.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("valid reqwest client config");
        Self {
            api_key: api_key.into(),
            model: model.into(),
            base_url: ANTHROPIC_BASE_URL.to_string(),
            http,
            thinking: None,
        }
    }

    /// Override the base URL (useful for testing with `httpmock`).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Enable extended thinking with the given token budget.
    ///
    /// Adds `ThinkingConfig { budget_tokens }` to every request and sends the
    /// `interleaved-thinking-2025-01-05` beta header.  Requires Claude 3.7 Sonnet
    /// or later.
    pub fn with_thinking(mut self, budget_tokens: u32) -> Self {
        self.thinking = Some(ThinkingConfig::new(budget_tokens));
        self
    }

    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.base_url.trim_end_matches('/'))
    }

    fn effective_model<'a>(&'a self, request_model: &'a str) -> &'a str {
        if request_model.is_empty() {
            self.model.as_str()
        } else {
            request_model
        }
    }
}

// ---------------------------------------------------------------------------
// Message / tool translation helpers
// ---------------------------------------------------------------------------

/// Extract the system prompt and translate the remaining messages for
/// the Anthropic API wire format.
///
/// Returns `(system, messages)` where `system` is the concatenated text of all
/// `Role::System` messages (Anthropic accepts a single top-level system field).
fn translate_messages(messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
    let system_parts: Vec<String> = messages
        .iter()
        .filter(|m| matches!(m.role, Role::System))
        .map(|m| m.content.to_text_lossy())
        .collect();

    let system = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n"))
    };

    let mut anthropic_messages: Vec<AnthropicMessage> = Vec::new();

    for message in messages {
        match message.role {
            Role::System => {
                // Handled above; skip from the messages array.
            }
            Role::User => {
                let content = match &message.content {
                    MessageContent::Text(t) => AnthropicContent::Text(t.clone()),
                    MessageContent::Parts(parts) => {
                        use crate::types::{AnthropicImageSource};
                        use wesichain_core::ContentPart;
                        let a_parts: Vec<AnthropicPart> = parts.iter().map(|p| match p {
                            ContentPart::Text { text } => AnthropicPart::Text { text: text.clone() },
                            ContentPart::ImageUrl { url, .. } => AnthropicPart::Image {
                                source: AnthropicImageSource {
                                    source_type: "url".to_string(),
                                    media_type: None,
                                    data: None,
                                    url: Some(url.clone()),
                                },
                            },
                            ContentPart::ImageData { data, media_type } => AnthropicPart::Image {
                                source: AnthropicImageSource {
                                    source_type: "base64".to_string(),
                                    media_type: Some(media_type.clone()),
                                    data: Some(data.clone()),
                                    url: None,
                                },
                            },
                        }).collect();
                        AnthropicContent::Parts(a_parts)
                    }
                };
                anthropic_messages.push(AnthropicMessage {
                    role: "user".to_string(),
                    content,
                });
            }
            Role::Assistant => {
                let mut parts: Vec<AnthropicPart> = Vec::new();

                if !message.content.is_empty() {
                    parts.push(AnthropicPart::Text {
                        text: message.content.to_text_lossy(),
                    });
                }

                for call in &message.tool_calls {
                    parts.push(AnthropicPart::ToolUse {
                        id: call.id.clone(),
                        name: call.name.clone(),
                        input: call.args.clone(),
                    });
                }

                if !parts.is_empty() {
                    anthropic_messages.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: AnthropicContent::Parts(parts),
                    });
                }
            }
            Role::Tool => {
                // Tool results are sent back as `role: "user"` messages.
                let tool_use_id = message
                    .tool_call_id
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());
                anthropic_messages.push(AnthropicMessage {
                    role: "user".to_string(),
                    content: AnthropicContent::Parts(vec![AnthropicPart::ToolResult {
                        tool_use_id,
                        content: message.content.to_text_lossy(),
                    }]),
                });
            }
        }
    }

    (system, anthropic_messages)
}

/// Convert [`ToolSpec`]s to Anthropic's `tools` array format.
fn translate_tools(tools: &[ToolSpec]) -> Vec<AnthropicTool> {
    tools
        .iter()
        .map(|t| AnthropicTool {
            name: t.name.clone(),
            description: t.description.clone(),
            input_schema: t.parameters.clone(),
        })
        .collect()
}

fn build_request(input: &LlmRequest, stream: bool, thinking: Option<ThinkingConfig>) -> AnthropicRequest {
    let (system, messages) = translate_messages(&input.messages);
    let tools = if input.tools.is_empty() {
        None
    } else {
        Some(translate_tools(&input.tools))
    };
    let stop_sequences = if input.stop_sequences.is_empty() {
        None
    } else {
        Some(input.stop_sequences.clone())
    };

    AnthropicRequest {
        model: input.model.clone(),
        max_tokens: input.max_tokens.unwrap_or(MAX_TOKENS_DEFAULT),
        messages,
        system,
        tools,
        stream,
        temperature: input.temperature,
        stop_sequences,
        thinking,
    }
}

fn content_blocks_to_response(
    blocks: Vec<ResponseContentBlock>,
    usage: TokenUsage,
    model: String,
) -> LlmResponse {
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for block in blocks {
        match block {
            ResponseContentBlock::Text { text } => {
                text_parts.push(text);
            }
            ResponseContentBlock::ToolUse { id, name, input } => {
                let args = if input.is_object() {
                    input
                } else {
                    serde_json::json!({ "value": input })
                };
                tool_calls.push(ToolCall { id, name, args });
            }
            // Thinking blocks are informational; we don't surface them in LlmResponse.
            ResponseContentBlock::Thinking { .. } => {}
        }
    }

    LlmResponse {
        content: text_parts.join(""),
        tool_calls,
        usage: Some(usage),
        model,
    }
}

// ---------------------------------------------------------------------------
// Runnable impl
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for AnthropicClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        let mut request = build_request(&input, false, self.thinking.clone());
        // Use the effective model (falling back to the client's default)
        request.model = self.effective_model(&input.model).to_string();

        let mut builder = self
            .http
            .post(self.messages_url())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json");
        if self.thinking.is_some() {
            builder = builder.header("anthropic-beta", "interleaved-thinking-2025-01-05");
        }
        let response = builder
            .json(&request)
            .send()
            .await
            .map_err(|err| WesichainError::LlmProvider(err.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<AnthropicErrorResponse>(&body)
                .map(|e| e.error.message)
                .unwrap_or_else(|_| format!("HTTP {}: {}", status, body));
            return Err(match status.as_u16() {
                401 | 403 => WesichainError::AuthenticationFailed {
                    provider: "anthropic".to_string(),
                    message,
                },
                429 => WesichainError::RateLimitExceeded { retry_after: None },
                _ => WesichainError::LlmProvider(message),
            });
        }

        let anthropic_response = response
            .json::<crate::types::AnthropicResponse>()
            .await
            .map_err(|err| WesichainError::LlmProvider(err.to_string()))?;

        let usage = TokenUsage {
            prompt_tokens: anthropic_response.usage.input_tokens,
            completion_tokens: anthropic_response.usage.output_tokens,
            total_tokens: anthropic_response.usage.input_tokens
                + anthropic_response.usage.output_tokens,
        };

        Ok(content_blocks_to_response(anthropic_response.content, usage, anthropic_response.model))
    }

    fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        let mut request = build_request(&input, true, self.thinking.clone());
        request.model = self.effective_model(&input.model).to_string();
        let has_thinking = self.thinking.is_some();

        let client = self.clone();

        stream::once(async move {
            let mut builder = client
                .http
                .post(client.messages_url())
                .header("x-api-key", &client.api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("content-type", "application/json");
            if has_thinking {
                builder = builder.header("anthropic-beta", "interleaved-thinking-2025-01-05");
            }
            builder
                .json(&request)
                .send()
                .await
                .map_err(|err| WesichainError::LlmProvider(err.to_string()))
        })
        .flat_map(|result| match result {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    parse_anthropic_stream(response)
                } else {
                    stream::once(async move {
                        let body = response.text().await.unwrap_or_default();
                        let message = serde_json::from_str::<AnthropicErrorResponse>(&body)
                            .map(|e| e.error.message)
                            .unwrap_or_else(|_| format!("HTTP {}: {}", status, body));
                        Err(match status.as_u16() {
                            401 | 403 => WesichainError::AuthenticationFailed {
                                provider: "anthropic".to_string(),
                                message,
                            },
                            429 => WesichainError::RateLimitExceeded { retry_after: None },
                            _ => WesichainError::LlmProvider(message),
                        })
                    })
                    .boxed()
                }
            }
            Err(err) => stream::iter(vec![Err(err)]).boxed(),
        })
        .boxed()
    }
}

impl wesichain_core::ToolCallingLlm for AnthropicClient {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use httpmock::prelude::*;
    use serde_json::json;
    use wesichain_core::{LlmRequest, Message, Role};

    fn make_client(server: &MockServer) -> AnthropicClient {
        AnthropicClient::new("test-api-key", "claude-3-5-sonnet-20241022")
            .with_base_url(server.base_url())
    }

    fn simple_user_request() -> LlmRequest {
        LlmRequest {
            model: String::new(),
            messages: vec![Message {
                role: Role::User,
                content: "Hello!".into(),
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            temperature: None,
            max_tokens: None,
            stop_sequences: vec![],
        }
    }

    // ------------------------------------------------------------------
    // Test 1 – non-streaming text response
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_non_streaming_text_response() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/messages")
                .header("x-api-key", "test-api-key")
                .header("anthropic-version", "2023-06-01");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "id": "msg_01",
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        { "type": "text", "text": "Hello, world!" }
                    ],
                    "model": "claude-3-5-sonnet-20241022",
                    "stop_reason": "end_turn",
                    "usage": { "input_tokens": 10, "output_tokens": 5 }
                }));
        });

        let client = make_client(&server);
        let response = client.invoke(simple_user_request()).await.unwrap();

        mock.assert();
        assert_eq!(response.content, "Hello, world!");
        assert!(response.tool_calls.is_empty());

        let usage = response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
    }

    // ------------------------------------------------------------------
    // Test 2 – non-streaming tool call
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_non_streaming_tool_call() {
        let server = MockServer::start();

        server.mock(|when, then| {
            when.method(POST).path("/v1/messages");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "id": "msg_02",
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "toolu_01",
                            "name": "get_weather",
                            "input": { "location": "London" }
                        }
                    ],
                    "model": "claude-3-5-sonnet-20241022",
                    "stop_reason": "tool_use",
                    "usage": { "input_tokens": 20, "output_tokens": 15 }
                }));
        });

        let client = make_client(&server);
        let response = client.invoke(simple_user_request()).await.unwrap();

        assert!(response.content.is_empty());
        assert_eq!(response.tool_calls.len(), 1);

        let call = &response.tool_calls[0];
        assert_eq!(call.id, "toolu_01");
        assert_eq!(call.name, "get_weather");
        assert_eq!(call.args["location"], "London");
    }

    // ------------------------------------------------------------------
    // Test 3 – system message extraction
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_system_message_extraction() {
        let server = MockServer::start();

        // The request body must contain the top-level "system" field.
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/messages")
                .json_body_partial(r#"{"system": "You are a helpful assistant."}"#);
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "id": "msg_03",
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "text", "text": "Sure!" }],
                    "model": "claude-3-5-sonnet-20241022",
                    "stop_reason": "end_turn",
                    "usage": { "input_tokens": 15, "output_tokens": 3 }
                }));
        });

        let client = make_client(&server);
        let request = LlmRequest {
            model: String::new(),
            messages: vec![
                Message {
                    role: Role::System,
                    content: "You are a helpful assistant.".into(),
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                Message {
                    role: Role::User,
                    content: "Can you help me?".into(),
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            tools: vec![],
            temperature: None,
            max_tokens: None,
            stop_sequences: vec![],
        };

        let response = client.invoke(request).await.unwrap();
        mock.assert();
        assert_eq!(response.content, "Sure!");
    }

    // ------------------------------------------------------------------
    // Test 4 – streaming text
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_streaming_text() {
        let server = MockServer::start();

        // Build a minimal valid Anthropic SSE stream
        let sse_body = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_04\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-3-5-sonnet-20241022\",\"stop_reason\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n",
            "\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n",
            "\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n",
            "\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\", world!\"}}\n",
            "\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n",
            "\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":7}}\n",
            "\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n",
            "\n",
        );

        server.mock(|when, then| {
            when.method(POST).path("/v1/messages");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(sse_body);
        });

        let client = make_client(&server);
        let events: Vec<_> = client
            .stream(simple_user_request())
            .collect()
            .await;

        let ok_events: Vec<StreamEvent> = events.into_iter().filter_map(|r| r.ok()).collect();

        let chunks: Vec<_> = ok_events
            .iter()
            .filter_map(|e| {
                if let StreamEvent::ContentChunk(t) = e {
                    Some(t.as_str())
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(chunks, vec!["Hello", ", world!"]);

        let final_answers: Vec<_> = ok_events
            .iter()
            .filter_map(|e| {
                if let StreamEvent::FinalAnswer(t) = e {
                    Some(t.as_str())
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(final_answers, vec!["Hello, world!"]);
    }
}
