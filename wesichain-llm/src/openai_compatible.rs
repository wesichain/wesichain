//! Generic OpenAI-compatible LLM client
//!
//! Supports any provider using OpenAI's API format (OpenAI, DeepSeek, Together, etc.)

use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

/// Request body for chat completions endpoint
#[derive(Serialize, Debug, Clone)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<crate::types::Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<crate::types::ToolSpec>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

/// Non-streaming response from chat completions
#[derive(Deserialize, Debug, Clone)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Choice {
    pub index: u32,
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ResponseMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<crate::types::ToolCall>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Streaming chunk (server-sent events)
#[derive(Deserialize, Debug, Clone)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Delta {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

/// OpenAI-style error response
#[derive(Deserialize, Debug, Clone)]
pub struct OpenAiError {
    pub error: ErrorDetail,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ErrorDetail {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub code: Option<String>,
}

use secrecy::{ExposeSecret, Secret};

/// Builder for OpenAiCompatibleClient
pub struct OpenAiCompatibleBuilder {
    base_url: Option<Url>,
    api_key: Option<Secret<String>>,
    default_model: Option<String>,
    timeout: Duration,
}

impl Default for OpenAiCompatibleBuilder {
    fn default() -> Self {
        Self {
            base_url: None,
            api_key: None,
            default_model: None,
            timeout: Duration::from_secs(60),
        }
    }
}

impl OpenAiCompatibleBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base_url(mut self, url: impl AsRef<str>) -> Result<Self, wesichain_core::WesichainError> {
        let url = Url::parse(url.as_ref())
            .map_err(|e| wesichain_core::WesichainError::InvalidConfig(format!("Invalid base URL: {}", e)))?;
        self.base_url = Some(url);
        Ok(self)
    }

    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(Secret::new(key.into()));
        self
    }

    pub fn default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn build(self) -> Result<OpenAiCompatibleClient, wesichain_core::WesichainError> {
        let base_url = self.base_url
            .ok_or_else(|| wesichain_core::WesichainError::InvalidConfig("base_url is required".to_string()))?;

        let api_key = self.api_key
            .ok_or_else(|| wesichain_core::WesichainError::InvalidConfig("api_key is required".to_string()))?;

        let http = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| wesichain_core::WesichainError::LlmProvider(format!("Failed to create HTTP client: {}", e)))?;

        Ok(OpenAiCompatibleClient {
            http,
            base_url,
            api_key,
            default_model: self.default_model.unwrap_or_default(),
            timeout: self.timeout,
        })
    }
}

use wesichain_core::{WesichainError};
use crate::{LlmRequest, LlmResponse};

use bytes::BytesMut;
use futures::{stream, StreamExt};

/// Parse a server-sent event line
fn parse_sse_line(line: &str) -> Option<&str> {
    let line = line.trim();
    if line.starts_with("data: ") {
        Some(&line[6..])
    } else {
        None
    }
}

/// Parse SSE stream into StreamEvents
fn parse_sse_stream(
    response: reqwest::Response,
) -> BoxStream<'static, Result<StreamEvent, WesichainError>> {
    let stream = response.bytes_stream();
    let mut buffer = BytesMut::new();

    stream
        .flat_map(move |chunk| {
            match chunk {
                Ok(bytes) => {
                    buffer.extend_from_slice(&bytes);
                    let mut events = Vec::new();

                    // Process complete lines in buffer
                    while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                        let line = buffer.split_to(pos + 1);
                        let line_str = String::from_utf8_lossy(&line);

                        if let Some(data) = parse_sse_line(&line_str) {
                            if data == "[DONE]" {
                                events.push(Ok(StreamEvent::FinalAnswer(String::new())));
                            } else if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                                for choice in chunk.choices {
                                    if let Some(content) = choice.delta.content {
                                        events.push(Ok(StreamEvent::ContentChunk(content)));
                                    }
                                }
                            }
                        }
                    }

                    stream::iter(events)
                }
                Err(e) => {
                    stream::iter(vec![Err(WesichainError::LlmProvider(format!("Stream error: {}", e)))])
                }
            }
        })
        .boxed()
}

/// Generic client for OpenAI-compatible APIs
#[derive(Clone)]
pub struct OpenAiCompatibleClient {
    http: reqwest::Client,
    base_url: Url,
    api_key: Secret<String>,
    default_model: String,
    timeout: Duration,
}

impl OpenAiCompatibleClient {
    pub fn builder() -> OpenAiCompatibleBuilder {
        OpenAiCompatibleBuilder::new()
    }

    /// Set or update the default model
    pub fn set_default_model(&mut self, model: impl Into<String>) {
        self.default_model = model.into();
    }

    /// Make a non-streaming chat completion request
    async fn chat_completion(&self,
        request: ChatCompletionRequest
    ) -> Result<ChatCompletionResponse, WesichainError> {
        let url = self.base_url.join("/v1/chat/completions")
            .map_err(|e| WesichainError::LlmProvider(format!("Invalid URL: {}", e)))?;

        let response = self.http
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key.expose_secret()))
            .json(&request)
            .send()
            .await
            .map_err(|e| WesichainError::LlmProvider(format!("Request failed: {}", e)))?;

        let status = response.status();

        if status.is_success() {
            response.json::<ChatCompletionResponse>().await
                .map_err(|e| WesichainError::LlmProvider(format!("Failed to parse response: {}", e)))
        } else {
            let error_text = response.text().await.unwrap_or_default();
            let error_msg = serde_json::from_str::<OpenAiError>(&error_text)
                .map(|e| e.error.message)
                .unwrap_or_else(|_| format!("HTTP {}: {}", status, error_text));

            Err(WesichainError::LlmProvider(error_msg))
        }
    }

    /// Make a streaming chat completion request
    async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest
    ) -> Result<BoxStream<'static, Result<StreamEvent, WesichainError>>, WesichainError> {
        let url = self.base_url.join("/v1/chat/completions")
            .map_err(|e| WesichainError::LlmProvider(format!("Invalid URL: {}", e)))?;

        let request = ChatCompletionRequest {
            stream: true,
            ..request
        };

        let response = self.http
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key.expose_secret()))
            .json(&request)
            .send()
            .await
            .map_err(|e| WesichainError::LlmProvider(format!("Request failed: {}", e)))?;

        let status = response.status();

        if status.is_success() {
            Ok(parse_sse_stream(response))
        } else {
            let error_text = response.text().await.unwrap_or_default();
            let error_msg = serde_json::from_str::<OpenAiError>(&error_text)
                .map(|e| e.error.message)
                .unwrap_or_else(|_| format!("HTTP {}: {}", status, error_text));

            Err(WesichainError::LlmProvider(error_msg))
        }
    }
}

use wesichain_core::{Runnable, StreamEvent};
use futures::stream::BoxStream;

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for OpenAiCompatibleClient {
    async fn invoke(&self,
        input: LlmRequest
    ) -> Result<LlmResponse, WesichainError> {
        let model = if input.model.is_empty() {
            self.default_model.clone()
        } else {
            input.model
        };

        let request = ChatCompletionRequest {
            model,
            messages: input.messages,
            tools: if input.tools.is_empty() { None } else { Some(input.tools) },
            temperature: None,
            max_tokens: None,
            stream: false,
        };

        let response = self.chat_completion(request).await?;

        let choice = response.choices.into_iter().next()
            .ok_or_else(|| WesichainError::LlmProvider("No choices in response".to_string()))?;

        Ok(LlmResponse {
            content: choice.message.content.unwrap_or_default(),
            tool_calls: choice.message.tool_calls.unwrap_or_default(),
        })
    }

    fn stream(&self,
        input: LlmRequest
    ) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        let model = if input.model.is_empty() {
            self.default_model.clone()
        } else {
            input.model
        };

        let request = ChatCompletionRequest {
            model,
            messages: input.messages,
            tools: if input.tools.is_empty() { None } else { Some(input.tools) },
            temperature: None,
            max_tokens: None,
            stream: true,
        };

        let client = self.clone();
        stream::once(async move {
            client.chat_completion_stream(request).await
        })
        .flat_map(|result| match result {
            Ok(stream) => stream,
            Err(e) => stream::iter(vec![Err(e)]).boxed(),
        })
        .boxed()
    }
}
