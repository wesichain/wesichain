# OpenAI and DeepSeek Provider Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add OpenAI and DeepSeek LLM providers to wesichain-llm using a generic OpenAI-compatible client with thin provider wrappers.

**Architecture:** A generic `OpenAiCompatibleClient` handles HTTP, SSE streaming, and error mapping. Thin wrappers `OpenAiClient` and `DeepSeekClient` provide provider-specific defaults (base URL, timeout, model). All implement the `Llm` trait.

**Tech Stack:** Rust, reqwest, secrecy, serde, futures, async-trait, tokio

---

## Prerequisites

- Worktree: `.worktrees/openai-deepseek-providers/`
- Branch: `feature/openai-deepseek-providers`
- Baseline tests passing

---

## Task 1: Add Dependencies to Cargo.toml

**Files:**
- Modify: `wesichain-llm/Cargo.toml`

**Step 1: Add new dependencies**

Add to `[dependencies]` section:

```toml
secrecy = "0.8"
url = "2"
```

Update `reqwest` to include rustls-tls:

```toml
reqwest = { version = "0.12", features = ["json", "stream", "rustls-tls"] }
```

**Step 2: Add feature flags**

Add to `[features]` section (create if doesn't exist):

```toml
[features]
default = ["ollama"]
openai = []
deepseek = []
ollama = []
all-providers = ["openai", "deepseek", "ollama"]
```

**Step 3: Verify Cargo.toml is valid**

```bash
cd wesichain-llm && cargo check --all-features 2>&1 | head -20
```

Expected: No errors about Cargo.toml syntax

**Step 4: Commit**

```bash
git add wesichain-llm/Cargo.toml
git commit -m "chore(deps): add secrecy, url deps and provider feature flags"
```

---

## Task 2: Create OpenAI-Compatible Types

**Files:**
- Create: `wesichain-llm/src/openai_compatible.rs`

**Step 1: Write the types module**

```rust
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
```

**Step 2: Verify compiles**

```bash
cd wesichain-llm && cargo check 2>&1 | head -20
```

Expected: No errors

**Step 3: Commit**

```bash
git add wesichain-llm/src/openai_compatible.rs
git commit -m "feat(llm): add OpenAI-compatible types (requests, responses, chunks)"
```

---

## Task 3: Implement OpenAiCompatibleBuilder

**Files:**
- Modify: `wesichain-llm/src/openai_compatible.rs`

**Step 1: Add builder struct and impl after types**

```rust
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
```

**Step 2: Verify compiles**

```bash
cd wesichain-llm && cargo check 2>&1 | head -20
```

Expected: No errors

**Step 3: Commit**

```bash
git add wesichain-llm/src/openai_compatible.rs
git commit -m "feat(llm): add OpenAiCompatibleBuilder with validation"
```

---

## Task 4: Implement OpenAiCompatibleClient (Non-Streaming)

**Files:**
- Modify: `wesichain-llm/src/openai_compatible.rs`

**Step 1: Add client struct and non-streaming implementation**

```rust
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use crate::{LlmRequest, LlmResponse};

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
    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, WesichainError> {
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
}
```

**Step 2: Verify compiles**

```bash
cd wesichain-llm && cargo check 2>&1 | head -20
```

Expected: No errors

**Step 3: Commit**

```bash
git add wesichain-llm/src/openai_compatible.rs
git commit -m "feat(llm): add OpenAiCompatibleClient with non-streaming chat completion"
```

---

## Task 5: Implement Runnable Trait (Non-Streaming)

**Files:**
- Modify: `wesichain-llm/src/openai_compatible.rs`

**Step 1: Add Runnable implementation**

```rust
use futures::stream::BoxStream;

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for OpenAiCompatibleClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
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

    fn stream(&self, _input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        // Placeholder - will implement in Task 6
        use futures::stream;
        stream::empty().boxed()
    }
}
```

**Step 2: Verify compiles**

```bash
cd wesichain-llm && cargo check 2>&1 | head -20
```

Expected: No errors

**Step 3: Commit**

```bash
git add wesichain-llm/src/openai_compatible.rs
git commit -m "feat(llm): implement Runnable trait for OpenAiCompatibleClient (non-streaming)"
```

---

## Task 6: Implement SSE Streaming Parser

**Files:**
- Modify: `wesichain-llm/src/openai_compatible.rs`

**Step 1: Add SSE parsing utilities**

```rust
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
    use tokio_stream::wrappers::BytesStream;

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
```

**Step 2: Add streaming method to client**

```rust
impl OpenAiCompatibleClient {
    /// Make a streaming chat completion request
    async fn chat_completion_stream(&self, request: ChatCompletionRequest) -> Result<BoxStream<'static, Result<StreamEvent, WesichainError>>, WesichainError> {
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
```

**Step 3: Update Runnable stream method**

```rust
fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
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
```

**Step 4: Verify compiles**

```bash
cd wesichain-llm && cargo check 2>&1 | head -30
```

Expected: No errors

**Step 5: Commit**

```bash
git add wesichain-llm/src/openai_compatible.rs
git commit -m "feat(llm): implement SSE streaming for OpenAiCompatibleClient"
```

---

## Task 7: Create Provider Wrappers Module

**Files:**
- Create: `wesichain-llm/src/providers/mod.rs`
- Create: `wesichain-llm/src/providers/openai.rs`
- Create: `wesichain-llm/src/providers/deepseek.rs`

**Step 1: Create providers/mod.rs**

```rust
//! Provider-specific LLM clients

#[cfg(feature = "openai")]
pub mod openai;

#[cfg(feature = "deepseek")]
pub mod deepseek;
```

**Step 2: Create providers/openai.rs**

```rust
//! OpenAI LLM client

use std::time::Duration;
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use futures::stream::BoxStream;
use crate::{LlmRequest, LlmResponse};
use crate::openai_compatible::OpenAiCompatibleClient;

/// OpenAI LLM client
///
/// # Example
/// ```
/// use wesichain_llm::providers::openai::OpenAiClient;
///
/// let client = OpenAiClient::new("sk-...")
///     .with_model("gpt-4o-mini");
/// ```
#[derive(Clone)]
pub struct OpenAiClient(OpenAiCompatibleClient);

impl OpenAiClient {
    /// Create a new OpenAI client with the given API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self(
            OpenAiCompatibleClient::builder()
                .base_url("https://api.openai.com")
                .expect("Valid URL")
                .api_key(api_key)
                .default_model("gpt-4o-mini")
                .timeout(Duration::from_secs(60))
                .build()
                .expect("Valid config")
        )
    }

    /// Set the model to use
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.0.set_default_model(model);
        self
    }
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for OpenAiClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        self.0.invoke(input).await
    }

    fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        self.0.stream(input)
    }
}

impl crate::Llm for OpenAiClient {}
```

**Step 3: Create providers/deepseek.rs**

```rust
//! DeepSeek LLM client

use std::time::Duration;
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use futures::stream::BoxStream;
use crate::{LlmRequest, LlmResponse};
use crate::openai_compatible::OpenAiCompatibleClient;

/// DeepSeek LLM client
///
/// # Example
/// ```
/// use wesichain_llm::providers::deepseek::DeepSeekClient;
///
/// let client = DeepSeekClient::new("sk-...")
///     .with_model("deepseek-chat");
/// ```
#[derive(Clone)]
pub struct DeepSeekClient(OpenAiCompatibleClient);

impl DeepSeekClient {
    /// Create a new DeepSeek client with the given API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self(
            OpenAiCompatibleClient::builder()
                .base_url("https://api.deepseek.com")
                .expect("Valid URL")
                .api_key(api_key)
                .default_model("deepseek-chat")
                .timeout(Duration::from_secs(300)) // DeepSeek reasoner can be slow
                .build()
                .expect("Valid config")
        )
    }

    /// Set the model to use
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.0.set_default_model(model);
        self
    }
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for DeepSeekClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        self.0.invoke(input).await
    }

    fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        self.0.stream(input)
    }
}

impl crate::Llm for DeepSeekClient {}
```

**Step 4: Verify compiles with features**

```bash
cd wesichain-llm && cargo check --features openai 2>&1 | head -20
cd wesichain-llm && cargo check --features deepseek 2>&1 | head -20
cd wesichain-llm && cargo check --features all-providers 2>&1 | head -20
```

Expected: No errors

**Step 5: Commit**

```bash
git add wesichain-llm/src/providers/
git commit -m "feat(llm): add OpenAiClient and DeepSeekClient provider wrappers"
```

---

## Task 8: Update lib.rs Exports

**Files:**
- Modify: `wesichain-llm/src/lib.rs`

**Step 1: Update lib.rs with new exports**

```rust
mod ollama;
mod types;

// OpenAI-compatible client (always available)
pub mod openai_compatible;

// Provider-specific clients (feature-gated)
pub mod providers;

pub use ollama::{ollama_stream_events, OllamaClient};
pub use types::{LlmRequest, LlmResponse, Message, Role, ToolCall, ToolSpec};

// Re-export generic client
pub use openai_compatible::{OpenAiCompatibleClient, OpenAiCompatibleBuilder, ChatCompletionRequest};

// Re-export provider clients
#[cfg(feature = "openai")]
pub use providers::openai::OpenAiClient;

#[cfg(feature = "deepseek")]
pub use providers::deepseek::DeepSeekClient;

use wesichain_core::Runnable;

pub trait Llm: Runnable<LlmRequest, LlmResponse> {}

impl<T> Llm for T where T: Runnable<LlmRequest, LlmResponse> {}
```

**Step 2: Verify compiles with all features**

```bash
cd wesichain-llm && cargo check --all-features 2>&1 | head -20
```

Expected: No errors

**Step 3: Commit**

```bash
git add wesichain-llm/src/lib.rs
git commit -m "feat(llm): update public API exports for OpenAI and DeepSeek providers"
```

---

## Task 9: Write Unit Tests for SSE Parser

**Files:**
- Create: `wesichain-llm/tests/sse_parsing.rs`

**Step 1: Write SSE parsing tests**

```rust
//! Unit tests for SSE (Server-Sent Events) parsing

use bytes::Bytes;
use futures::StreamExt;

// Helper to simulate SSE stream
fn create_sse_stream(data: Vec<&str>) -> impl futures::Stream<Item = Result<Bytes, reqwest::Error>> {
    futures::stream::iter(data.into_iter().map(|s| Ok(Bytes::from(s))))
}

#[test]
fn test_parse_sse_line() {
    // Test data line extraction
    assert_eq!(
        parse_sse_line("data: {\"key\": \"value\"}"),
        Some("{\"key\": \"value\"}")
    );

    // Test empty data
    assert_eq!(parse_sse_line("data: "), Some(""));

    // Test non-data lines are ignored
    assert_eq!(parse_sse_line("event: message"), None);
    assert_eq!(parse_sse_line(": comment"), None);
    assert_eq!(parse_sse_line(""), None);
}

#[test]
fn test_parse_sse_line_with_whitespace() {
    assert_eq!(
        parse_sse_line("  data: {\"key\": \"value\"}  "),
        Some("{\"key\": \"value\"}")
    );
}

// Import the parse_sse_line function from the crate
// This requires making it pub(crate) or testing via public API
use wesichain_llm::openai_compatible::*;

#[tokio::test]
async fn test_chat_completion_request_serialization() {
    let request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
        tools: None,
        temperature: Some(0.7),
        max_tokens: Some(100),
        stream: false,
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("\"model\":\"gpt-4\""));
    assert!(json.contains("\"temperature\":0.7"));
    assert!(json.contains("\"max_tokens\":100"));
    assert!(json.contains("\"stream\":false"));
}

#[test]
fn test_chat_completion_response_deserialization() {
    let json = r#"{
        "id": "chat-123",
        "object": "chat.completion",
        "created": 1234567890,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hello!"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    }"#;

    let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.id, "chat-123");
    assert_eq!(response.model, "gpt-4");
    assert_eq!(response.choices.len(), 1);
    assert_eq!(response.choices[0].message.content, Some("Hello!".to_string()));
    assert_eq!(response.usage.unwrap().total_tokens, 15);
}

#[test]
fn test_chat_completion_chunk_deserialization() {
    let json = r#"{
        "id": "chatcmpl-123",
        "object": "chat.completion.chunk",
        "created": 1234567890,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "delta": {
                "content": "Hello"
            },
            "finish_reason": null
        }]
    }"#;

    let chunk: ChatCompletionChunk = serde_json::from_str(json).unwrap();
    assert_eq!(chunk.id, "chatcmpl-123");
    assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
}

#[test]
fn test_error_response_deserialization() {
    let json = r#"{
        "error": {
            "message": "Invalid API key",
            "type": "authentication_error",
            "code": "invalid_api_key"
        }
    }"#;

    let error: OpenAiError = serde_json::from_str(json).unwrap();
    assert_eq!(error.error.message, "Invalid API key");
    assert_eq!(error.error.error_type, Some("authentication_error".to_string()));
    assert_eq!(error.error.code, Some("invalid_api_key".to_string()));
}
```

**Step 2: Run tests**

```bash
cd wesichain-llm && cargo test --all-features 2>&1 | tail -30
```

Expected: Tests pass

**Step 3: Commit**

```bash
git add wesichain-llm/tests/sse_parsing.rs
git commit -m "test(llm): add unit tests for SSE parsing and type serialization"
```

---

## Task 10: Write Integration Tests

**Files:**
- Create: `wesichain-llm/tests/openai_integration.rs`
- Create: `wesichain-llm/tests/deepseek_integration.rs`

**Step 1: Create openai_integration.rs**

```rust
//! Integration tests for OpenAI provider
//!
//! Run with: cargo test --features openai -- --ignored

use wesichain_llm::{OpenAiClient, LlmRequest, Message, Role, Llm};
use wesichain_core::Runnable;

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY environment variable"]
async fn test_openai_simple_completion() {
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let client = OpenAiClient::new(api_key);

    let request = LlmRequest {
        model: "gpt-4o-mini".to_string(),
        messages: vec![
            Message {
                role: Role::User,
                content: "Say 'Hello from Wesichain' and nothing else".to_string(),
                tool_call_id: None,
            }
        ],
        tools: vec![],
    };

    let response = client.invoke(request).await.expect("Request failed");
    assert!(response.content.contains("Hello") || response.content.contains("Wesichain"));
    println!("Response: {}", response.content);
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY environment variable"]
async fn test_openai_streaming() {
    use futures::StreamExt;

    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let client = OpenAiClient::new(api_key);

    let request = LlmRequest {
        model: "gpt-4o-mini".to_string(),
        messages: vec![
            Message {
                role: Role::User,
                content: "Count from 1 to 5".to_string(),
                tool_call_id: None,
            }
        ],
        tools: vec![],
    };

    let mut stream = client.stream(request);
    let mut content = String::new();

    while let Some(event) = stream.next().await {
        match event {
            Ok(wesichain_core::StreamEvent::ContentChunk(chunk)) => {
                content.push_str(&chunk);
                print!("{}", chunk);
            }
            Ok(wesichain_core::StreamEvent::FinalAnswer(_)) => break,
            Err(e) => panic!("Stream error: {}", e),
            _ => {}
        }
    }

    assert!(!content.is_empty());
    println!("\nFull content: {}", content);
}
```

**Step 2: Create deepseek_integration.rs**

```rust
//! Integration tests for DeepSeek provider
//!
//! Run with: cargo test --features deepseek -- --ignored

use wesichain_llm::{DeepSeekClient, LlmRequest, Message, Role, Llm};
use wesichain_core::Runnable;

#[tokio::test]
#[ignore = "Requires DEEPSEEK_API_KEY environment variable"]
async fn test_deepseek_simple_completion() {
    let api_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY not set");
    let client = DeepSeekClient::new(api_key);

    let request = LlmRequest {
        model: "deepseek-chat".to_string(),
        messages: vec![
            Message {
                role: Role::User,
                content: "Say 'Hello from DeepSeek via Wesichain' and nothing else".to_string(),
                tool_call_id: None,
            }
        ],
        tools: vec![],
    };

    let response = client.invoke(request).await.expect("Request failed");
    assert!(response.content.contains("Hello") || response.content.contains("DeepSeek") || response.content.contains("Wesichain"));
    println!("Response: {}", response.content);
}

#[tokio::test]
#[ignore = "Requires DEEPSEEK_API_KEY environment variable"]
async fn test_deepseek_streaming() {
    use futures::StreamExt;

    let api_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY not set");
    let client = DeepSeekClient::new(api_key);

    let request = LlmRequest {
        model: "deepseek-chat".to_string(),
        messages: vec![
            Message {
                role: Role::User,
                content: "What is 2+2? Answer in one word.".to_string(),
                tool_call_id: None,
            }
        ],
        tools: vec![],
    };

    let mut stream = client.stream(request);
    let mut content = String::new();

    while let Some(event) = stream.next().await {
        match event {
            Ok(wesichain_core::StreamEvent::ContentChunk(chunk)) => {
                content.push_str(&chunk);
                print!("{}", chunk);
            }
            Ok(wesichain_core::StreamEvent::FinalAnswer(_)) => break,
            Err(e) => panic!("Stream error: {}", e),
            _ => {}
        }
    }

    assert!(!content.is_empty());
    println!("\nFull content: {}", content);
}

#[tokio::test]
#[ignore = "Requires DEEPSEEK_API_KEY environment variable"]
async fn test_deepseek_reasoner() {
    let api_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY not set");
    let client = DeepSeekClient::new(api_key)
        .with_model("deepseek-reasoner");

    let request = LlmRequest {
        model: "deepseek-reasoner".to_string(),
        messages: vec![
            Message {
                role: Role::User,
                content: "Solve: If a train travels 60 km in 30 minutes, what is its average speed in km/h?".to_string(),
                tool_call_id: None,
            }
        ],
        tools: vec![],
    };

    let response = client.invoke(request).await.expect("Request failed");
    assert!(!response.content.is_empty());
    println!("Reasoner response: {}", response.content);
}
```

**Step 3: Run unit tests (not integration)**

```bash
cd wesichain-llm && cargo test --all-features 2>&1 | tail -20
```

Expected: Unit tests pass, integration tests skipped

**Step 4: Commit**

```bash
git add wesichain-llm/tests/openai_integration.rs wesichain-llm/tests/deepseek_integration.rs
git commit -m "test(llm): add integration tests for OpenAI and DeepSeek providers"
```

---

## Task 11: Update Workspace Cargo.toml

**Files:**
- Modify: `wesichain-llm/Cargo.toml` (add tokio-stream dependency)

**Step 1: Add tokio-stream dependency**

Add to `[dependencies]`:

```toml
tokio-stream = "0.1"
```

**Step 2: Verify workspace builds**

```bash
cargo build --all-features 2>&1 | tail -20
```

Expected: No errors

**Step 3: Commit**

```bash
git add wesichain-llm/Cargo.toml
git commit -m "chore(deps): add tokio-stream for SSE parsing"
```

---

## Task 12: Final Verification and Documentation

**Step 1: Run full test suite**

```bash
cargo test --all-features 2>&1 | tail -30
```

Expected: All unit tests pass

**Step 2: Check formatting**

```bash
cargo fmt -- --check 2>&1
```

Expected: No formatting issues (or run `cargo fmt` to fix)

**Step 3: Run clippy**

```bash
cargo clippy --all-features 2>&1 | head -30
```

Expected: No warnings (or fix them)

**Step 4: Build release**

```bash
cargo build --release --all-features 2>&1 | tail -10
```

Expected: Successful build

**Step 5: Commit any fixes**

```bash
git add -A
git commit -m "style: apply formatting and clippy fixes" || echo "No changes to commit"
```

---

## Summary

After completing all tasks:

- **Generic client**: `OpenAiCompatibleClient` with builder pattern
- **Providers**: `OpenAiClient`, `DeepSeekClient` (thin wrappers)
- **Features**: `openai`, `deepseek`, `ollama`, `all-providers`
- **Streaming**: SSE parsing for real-time responses
- **Tests**: Unit tests + integration tests (require API keys)

**Usage:**

```rust
use wesichain_llm::{OpenAiClient, DeepSeekClient, Llm};

// OpenAI
let openai = OpenAiClient::new("sk-...").with_model("gpt-4o");
let response = openai.invoke(request).await?;

// DeepSeek
let deepseek = DeepSeekClient::new("sk-...").with_model("deepseek-reasoner");
let response = deepseek.invoke(request).await?;
```

**Next Steps:**
1. Run integration tests with real API keys
2. Add more providers (Together, Groq, Fireworks) following same pattern
3. Add tool-calling streaming support
