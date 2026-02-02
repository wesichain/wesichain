# Wesichain Phase 2 LLM + Agent Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver a runnable end-to-end tool-calling agent using Ollama with streaming, prompt templates, and a minimal tool registry.

**Architecture:** Add `wesichain-llm` types and an Ollama provider implementing `Runnable<LlmRequest, LlmResponse>` with streaming; add `wesichain-prompt` templates as a runnable; add `wesichain-agent` tool trait/registry and a minimal ReAct-style loop. Use a robust NDJSON parser for streaming (buffer + serde_json StreamDeserializer), inject tool specs into LLM requests, and include `tool_call_id` in tool result messages. Keep core behavior simple and typed, with tests for serialization, parsing, and loop control.

**Note:** Keep `async-trait` in Phase 2 to match the core `Runnable` and avoid object-safety regressions; revisit native async traits in Phase 3+.

**Tech Stack:** Rust 1.75+, reqwest, serde/serde_json, tokio, futures, httpmock (tests), async-trait, regex.

**Skills:** Use @superpowers:executing-plans for implementation; use @superpowers:test-driven-development and @superpowers:verification-before-completion.

---

### Task 1: Add LLM message and tool types

**Files:**
- Create: `wesichain-llm/src/types.rs`
- Modify: `wesichain-llm/src/lib.rs`
- Modify: `wesichain-llm/Cargo.toml`
- Test: `wesichain-llm/tests/types.rs`

**Step 1: Write the failing test**

```rust
use serde_json::json;
use wesichain_llm::{LlmRequest, Message, Role, ToolSpec};

#[test]
fn llm_request_serializes_with_tools() {
    let req = LlmRequest {
        model: "llama3.1".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "hi".to_string(),
            tool_call_id: None,
        }],
        tools: vec![ToolSpec {
            name: "calculator".to_string(),
            description: "math".to_string(),
            parameters: json!({"type":"object","properties":{}}),
        }],
    };

    let value = serde_json::to_value(req).expect("serialize");
    assert_eq!(value["model"], "llama3.1");
    assert_eq!(value["messages"][0]["role"], "user");
    assert_eq!(value["tools"][0]["name"], "calculator");

    let tool_msg = Message {
        role: Role::Tool,
        content: "ok".to_string(),
        tool_call_id: Some("call-1".to_string()),
    };
    let tool_value = serde_json::to_value(tool_msg).expect("serialize tool msg");
    assert_eq!(tool_value["tool_call_id"], "call-1");
}

#[test]
fn llm_response_serializes_with_content_and_tool_calls_only() {
    let response = LlmResponse {
        content: "hello".to_string(),
        tool_calls: vec![ToolCall {
            id: "call-1".to_string(),
            name: "calculator".to_string(),
            args: json!({"x": 1}),
        }],
    };

    let value = serde_json::to_value(response).expect("serialize response");
    assert_eq!(value["content"], "hello");
    assert!(value.get("tool_calls").is_some());
}

#[test]
fn llm_response_omits_tool_calls_when_empty() {
    let response = LlmResponse {
        content: "hello".to_string(),
        tool_calls: vec![],
    };

    let value = serde_json::to_value(response).expect("serialize response");
    assert!(value.get("tool_calls").is_none());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-llm --test types -v`
Expected: FAIL with unresolved imports or missing types.

**Step 3: Write minimal implementation**

```rust
// wesichain-llm/src/types.rs
use serde::{Deserialize, Serialize};
use wesichain_core::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmResponse {
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}
```

```rust
// wesichain-llm/src/lib.rs
mod types;

pub use types::{LlmRequest, LlmResponse, Message, Role, ToolCall, ToolSpec};
```

```toml
# wesichain-llm/Cargo.toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
wesichain-core = { path = "../wesichain-core" }
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-llm --test types -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-llm/src/types.rs wesichain-llm/src/lib.rs wesichain-llm/Cargo.toml wesichain-llm/tests/types.rs
git commit -m "feat(llm): add request/response types"
```

---

### Task 2: Define Llm trait (typed Runnable alias)

**Files:**
- Modify: `wesichain-llm/src/lib.rs`
- Test: `wesichain-llm/tests/llm_trait.rs`

**Step 1: Write the failing test**

```rust
use async_trait::async_trait;
use wesichain_core::WesichainError;
use wesichain_llm::{Llm, LlmRequest, LlmResponse};

struct DummyLlm;

#[async_trait]
impl wesichain_core::Runnable<LlmRequest, LlmResponse> for DummyLlm {
    async fn invoke(&self, _input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        Ok(LlmResponse { content: "ok".to_string(), tool_calls: vec![] })
    }

    fn stream(
        &self,
        _input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

fn assert_llm<T: Llm>() {}

#[test]
fn dummy_llm_implements_llm() {
    assert_llm::<DummyLlm>();
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-llm --test llm_trait -v`
Expected: FAIL with missing `Llm` trait.

**Step 3: Write minimal implementation**

```rust
// wesichain-llm/src/lib.rs
use wesichain_core::Runnable;

pub trait Llm: Runnable<LlmRequest, LlmResponse> {}

impl<T> Llm for T where T: Runnable<LlmRequest, LlmResponse> {}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-llm --test llm_trait -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-llm/src/lib.rs wesichain-llm/tests/llm_trait.rs
git commit -m "feat(llm): add Llm trait"
```

---

### Task 3: Implement Ollama client (invoke)

**Files:**
- Create: `wesichain-llm/src/ollama.rs`
- Modify: `wesichain-llm/src/lib.rs`
- Modify: `wesichain-llm/Cargo.toml`
- Test: `wesichain-llm/tests/ollama_invoke.rs`

**Step 1: Write the failing test**

```rust
use httpmock::prelude::*;
use serde_json::json;
use wesichain_llm::{LlmRequest, Message, OllamaClient, Role};

#[tokio::test]
async fn ollama_invoke_maps_response() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/api/chat");
        then.status(200).json_body(json!({
            "message": {"content": "hello"},
            "done": true,
            "tool_calls": []
        }));
    });

    let client = OllamaClient::new(server.url(""), "llama3.1".to_string()).expect("client");
    let req = LlmRequest {
        model: "llama3.1".to_string(),
        messages: vec![Message { role: Role::User, content: "hi".to_string(), tool_call_id: None }],
        tools: vec![],
    };

    let resp = client.invoke(req).await.expect("invoke");
    assert_eq!(resp.content, "hello");
    mock.assert();
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-llm --test ollama_invoke -v`
Expected: FAIL with missing `OllamaClient`.

**Step 3: Write minimal implementation**

```rust
// wesichain-llm/src/ollama.rs
use reqwest::Client;
use serde::{Deserialize, Serialize};

use wesichain_core::{Runnable, StreamEvent, WesichainError};

use crate::{LlmRequest, LlmResponse};

#[derive(Clone)]
pub struct OllamaClient {
    base_url: String,
    model: String,
    http: Client,
}

impl OllamaClient {
    pub fn new(base_url: String, model: String) -> Result<Self, WesichainError> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| WesichainError::LlmProvider(e.to_string()))?;
        Ok(Self { base_url, model, http })
    }
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<crate::Message>,
    #[serde(default)]
    tools: Vec<crate::ToolSpec>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
    #[serde(default)]
    tool_calls: Vec<crate::ToolCall>,
    done: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for OllamaClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: input.messages,
            tools: input.tools,
            stream: false,
        };

        let url = format!("{}/api/chat", self.base_url);
        let response: OllamaChatResponse = self.http.post(url).json(&request).send().await
            .map_err(|e| WesichainError::LlmProvider(e.to_string()))?
            .json().await
            .map_err(|e| WesichainError::LlmProvider(e.to_string()))?;

        Ok(LlmResponse { content: response.message.content, tool_calls: response.tool_calls })
    }

    fn stream(&self, _input: LlmRequest) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}
```

```rust
// wesichain-llm/src/lib.rs
mod ollama;

pub use ollama::OllamaClient;
```

```toml
# wesichain-llm/Cargo.toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "stream"] }
async-trait = "0.1"
futures = "0.3"
bytes = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

[dev-dependencies]
httpmock = "0.7"
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-llm --test ollama_invoke -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-llm/src/ollama.rs wesichain-llm/src/lib.rs wesichain-llm/Cargo.toml wesichain-llm/tests/ollama_invoke.rs
git commit -m "feat(llm): add Ollama invoke"
```

---

### Task 4: Add Ollama streaming parser

**Files:**
- Modify: `wesichain-llm/src/ollama.rs`
- Test: `wesichain-llm/tests/ollama_stream.rs`

**Step 1: Write the failing test**

```rust
use wesichain_llm::ollama_stream_events;
use wesichain_core::StreamEvent;

#[test]
fn parse_stream_lines_into_events() {
    let input = br#"{\"message\":{\"content\":\"Hel\"},\"done\":false}\n{\"message\":{\"content\":\"lo\"},\"done\":false}\n{\"message\":{\"content\":\"!\"},\"done\":true}"#;
    let events = ollama_stream_events(input).expect("parse");
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], StreamEvent::ContentChunk(_)));
    assert!(matches!(events[2], StreamEvent::FinalAnswer(_)));
}

#[test]
fn parse_stream_rejects_malformed_json() {
    let bad = br#"{\"message\":{\"content\":\"hi\"}"#;
    assert!(ollama_stream_events(bad).is_err());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-llm --test ollama_stream -v`
Expected: FAIL with missing `ollama_stream_events`.

**Step 3: Write minimal implementation**

```rust
// wesichain-llm/src/ollama.rs
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OllamaStreamChunk {
    message: OllamaMessage,
    done: bool,
}

pub fn ollama_stream_events(input: &[u8]) -> Result<Vec<StreamEvent>, WesichainError> {
    let mut events = Vec::new();
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let iter = deserializer.into_iter::<OllamaStreamChunk>();
    for item in iter {
        let chunk = item.map_err(|err| WesichainError::ParseFailed {
            output: String::from_utf8_lossy(input).to_string(),
            reason: err.to_string(),
        })?;
        if chunk.done {
            events.push(StreamEvent::FinalAnswer(chunk.message.content));
        } else {
            events.push(StreamEvent::ContentChunk(chunk.message.content));
        }
    }
    Ok(events)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-llm --test ollama_stream -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-llm/src/ollama.rs wesichain-llm/tests/ollama_stream.rs
git commit -m "feat(llm): add Ollama stream parser"
```

---

### Task 5: Implement Ollama streaming invoke

**Files:**
- Modify: `wesichain-llm/src/ollama.rs`
- Test: `wesichain-llm/tests/ollama_stream_invoke.rs`

**Step 1: Write the failing test**

```rust
use httpmock::prelude::*;
use wesichain_llm::{LlmRequest, Message, OllamaClient, Role};
use futures::StreamExt;

#[tokio::test]
async fn ollama_stream_emits_events() {
    let server = MockServer::start();
    let body = "{\"message\":{\"content\":\"Hel\"},\"done\":false}\n{\"message\":{\"content\":\"lo\"},\"done\":true}";
    server.mock(|when, then| {
        when.method(POST).path("/api/chat");
        then.status(200).body(body).header("content-type", "application/x-ndjson");
    });

    let client = OllamaClient::new(server.url(""), "llama3.1".to_string()).expect("client");
    let req = LlmRequest {
        model: "llama3.1".to_string(),
        messages: vec![Message { role: Role::User, content: "hi".to_string(), tool_call_id: None }],
        tools: vec![],
    };

    let mut events = client.stream(req);
    let first = events.next().await.expect("event").expect("ok");
    assert!(matches!(first, wesichain_core::StreamEvent::ContentChunk(_)));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-llm --test ollama_stream_invoke -v`
Expected: FAIL with missing stream implementation.

**Step 3: Write minimal implementation**

```rust
// wesichain-llm/src/ollama.rs
use bytes::{Buf, BytesMut};
use futures::{stream, StreamExt};
use serde_json::error::Category;

fn parse_ndjson_buffer(
    buffer: &mut BytesMut,
) -> Result<Vec<StreamEvent>, WesichainError> {
    let mut events = Vec::new();
    let mut de = serde_json::Deserializer::from_slice(buffer);
    let mut iter = de.into_iter::<OllamaStreamChunk>();
    let mut consumed = 0;

    while let Some(item) = iter.next() {
        match item {
            Ok(chunk) => {
                consumed = iter.byte_offset();
                if chunk.done {
                    events.push(StreamEvent::FinalAnswer(chunk.message.content));
                } else {
                    events.push(StreamEvent::ContentChunk(chunk.message.content));
                }
            }
            Err(err) => {
                if err.classify() == Category::Eof {
                    break;
                }
                return Err(WesichainError::ParseFailed {
                    output: String::from_utf8_lossy(buffer).to_string(),
                    reason: err.to_string(),
                });
            }
        }
    }

    buffer.advance(consumed);
    Ok(events)
}

fn stream_from_ndjson(
    response: reqwest::Response,
) -> futures::stream::BoxStream<'static, Result<StreamEvent, WesichainError>> {
    let mut buffer = BytesMut::new();
    response
        .bytes_stream()
        .flat_map(move |chunk| match chunk {
            Ok(bytes) => {
                buffer.extend_from_slice(&bytes);
                match parse_ndjson_buffer(&mut buffer) {
                    Ok(events) => stream::iter(events.into_iter().map(Ok).collect::<Vec<_>>()),
                    Err(err) => stream::iter(vec![Err(err)]),
                }
            }
            Err(err) => stream::iter(vec![Err(WesichainError::LlmProvider(err.to_string()))]),
        })
        .boxed()
}

fn stream(&self, input: LlmRequest) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
    let request = OllamaChatRequest { model: self.model.clone(), messages: input.messages, tools: input.tools, stream: true };
    let url = format!("{}/api/chat", self.base_url);
    let fut = self.http.post(url).json(&request).send();
    let stream = stream::once(async move {
        fut.await.map_err(|e| WesichainError::LlmProvider(e.to_string()))
    })
    .flat_map(|result| match result {
        Ok(resp) => stream_from_ndjson(resp),
        Err(err) => stream::iter(vec![Err(err)]),
    });
    stream.boxed()
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-llm --test ollama_stream_invoke -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-llm/src/ollama.rs wesichain-llm/tests/ollama_stream_invoke.rs
git commit -m "feat(llm): add Ollama streaming invoke"
```

---

### Task 6: Implement prompt template runnable

**Files:**
- Create: `wesichain-prompt/src/template.rs`
- Modify: `wesichain-prompt/src/lib.rs`
- Modify: `wesichain-prompt/Cargo.toml`
- Test: `wesichain-prompt/tests/template.rs`

**Step 1: Write the failing test**

```rust
use std::collections::HashMap;
use wesichain_prompt::PromptTemplate;
use wesichain_core::Value;

#[test]
fn renders_template_with_vars() {
    let tmpl = PromptTemplate::new("Hello {{name}}".to_string());
    let mut vars = HashMap::new();
    vars.insert("name".to_string(), Value::from("Wesi"));
    let rendered = tmpl.render(&vars).expect("render");
    assert_eq!(rendered, "Hello Wesi");
}

#[test]
fn does_not_confuse_overlapping_keys() {
    let tmpl = PromptTemplate::new("{{name}} {{fullname}}".to_string());
    let mut vars = HashMap::new();
    vars.insert("name".to_string(), Value::from("X"));
    vars.insert("fullname".to_string(), Value::from("Y"));
    let rendered = tmpl.render(&vars).expect("render");
    assert_eq!(rendered, "X Y");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-prompt --test template -v`
Expected: FAIL with missing `PromptTemplate`.

**Step 3: Write minimal implementation**

```rust
// wesichain-prompt/src/template.rs
use std::collections::HashMap;

use regex::Regex;
use wesichain_core::{Value, WesichainError};

#[derive(Debug, Clone)]
pub struct PromptTemplate {
    template: String,
}

impl PromptTemplate {
    pub fn new(template: String) -> Self {
        Self { template }
    }

    pub fn render(&self, vars: &HashMap<String, Value>) -> Result<String, WesichainError> {
        let pattern = Regex::new(r"\{\{\s*(\w+)\s*\}\}")
            .map_err(|e| WesichainError::InvalidConfig(e.to_string()))?;
        let rendered = pattern.replace_all(&self.template, |caps: &regex::Captures| {
            let key = &caps[1];
            match vars.get(key) {
                Some(value) => value.as_str().map(|s| s.to_string()).unwrap_or_else(|| value.to_string()),
                None => "".to_string(),
            }
        });
        Ok(rendered.to_string())
    }
}
```

```rust
// wesichain-prompt/src/lib.rs
mod template;

pub use template::PromptTemplate;
```

```toml
# wesichain-prompt/Cargo.toml
[dependencies]
wesichain-core = { path = "../wesichain-core" }
regex = "1"
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-prompt --test template -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-prompt/src/template.rs wesichain-prompt/src/lib.rs wesichain-prompt/Cargo.toml wesichain-prompt/tests/template.rs
git commit -m "feat(prompt): add PromptTemplate"
```

---

### Task 7: Add Tool trait and registry

**Files:**
- Create: `wesichain-agent/src/tool.rs`
- Modify: `wesichain-agent/src/lib.rs`
- Modify: `wesichain-agent/Cargo.toml`
- Test: `wesichain-agent/tests/tool_registry.rs`

**Step 1: Write the failing test**

```rust
use std::collections::HashMap;
use wesichain_agent::{Tool, ToolRegistry};
use wesichain_core::{Value, WesichainError};
use wesichain_llm::ToolSpec;

struct EchoTool;

#[async_trait::async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str { "echo" }
    fn description(&self) -> &str { "echoes" }
    fn schema(&self) -> Value { Value::from("schema") }

    async fn call(&self, input: Value) -> Result<Value, WesichainError> {
        Ok(input)
    }
}

#[tokio::test]
async fn registry_calls_tool() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(EchoTool));
    let output = registry.call("echo", Value::from("hi")).await.unwrap();
    assert_eq!(output, Value::from("hi"));

    let specs: Vec<ToolSpec> = registry.to_specs();
    assert_eq!(specs.len(), 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-agent --test tool_registry -v`
Expected: FAIL with missing `Tool` or `ToolRegistry`.

**Step 3: Write minimal implementation**

```rust
// wesichain-agent/src/tool.rs
use std::collections::HashMap;

use wesichain_core::{Value, WesichainError};
use wesichain_llm::ToolSpec;

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    async fn call(&self, input: Value) -> Result<Value, WesichainError>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub async fn call(&self, name: &str, input: Value) -> Result<Value, WesichainError> {
        let tool = self.tools.get(name).ok_or_else(|| WesichainError::ToolCallFailed {
            tool_name: name.to_string(),
            reason: "not found".to_string(),
        })?;
        tool.call(input).await
    }

    pub fn to_specs(&self) -> Vec<ToolSpec> {
        self.tools
            .values()
            .map(|tool| ToolSpec {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.schema(),
            })
            .collect()
    }
}
```

```rust
// wesichain-agent/src/lib.rs
mod tool;

pub use tool::{Tool, ToolRegistry};
```

```toml
# wesichain-agent/Cargo.toml
[dependencies]
async-trait = "0.1"
wesichain-core = { path = "../wesichain-core" }
wesichain-llm = { path = "../wesichain-llm" }
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-agent --test tool_registry -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-agent/src/tool.rs wesichain-agent/src/lib.rs wesichain-agent/Cargo.toml wesichain-agent/tests/tool_registry.rs
git commit -m "feat(agent): add Tool registry"
```

---

### Task 8: Implement ToolCallingAgent loop

**Files:**
- Create: `wesichain-agent/src/agent.rs`
- Modify: `wesichain-agent/src/lib.rs`
- Modify: `wesichain-agent/Cargo.toml`
- Test: `wesichain-agent/tests/agent_loop.rs`

**Step 1: Write the failing test**

```rust
use async_trait::async_trait;
use wesichain_agent::{Tool, ToolCallingAgent, ToolRegistry};
use wesichain_core::{Runnable, StreamEvent, Value, WesichainError};
use wesichain_llm::{LlmRequest, LlmResponse, Message, Role, ToolCall, ToolSpec};

struct MockLlm;

#[async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockLlm {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        if input.messages.len() == 1 {
            return Ok(LlmResponse {
                content: "".to_string(),
                tool_calls: vec![ToolCall { id: "1".to_string(), name: "echo".to_string(), args: Value::from("hi") }],
            });
        }
        Ok(LlmResponse { content: "done".to_string(), tool_calls: vec![] })
    }

    fn stream(&self, _input: LlmRequest) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str { "echo" }
    fn description(&self) -> &str { "echo" }
    fn schema(&self) -> Value { Value::from("schema") }
    async fn call(&self, input: Value) -> Result<Value, WesichainError> { Ok(input) }
}

#[tokio::test]
async fn agent_calls_tool_then_finishes() {
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(EchoTool));

    let agent = ToolCallingAgent::new(MockLlm, tools, "mock".to_string()).max_steps(3);
    let output = agent.invoke("hi".to_string()).await.unwrap();
    assert_eq!(output, "done");
}

#[tokio::test]
async fn agent_stops_after_max_steps() {
    struct LoopLlm;

    #[async_trait]
    impl Runnable<LlmRequest, LlmResponse> for LoopLlm {
        async fn invoke(&self, _input: LlmRequest) -> Result<LlmResponse, WesichainError> {
            Ok(LlmResponse {
                content: "".to_string(),
                tool_calls: vec![ToolCall { id: "1".to_string(), name: "echo".to_string(), args: Value::from("hi") }],
            })
        }

        fn stream(&self, _input: LlmRequest) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
            futures::stream::empty().boxed()
        }
    }

    let mut tools = ToolRegistry::new();
    tools.register(Box::new(EchoTool));

    let agent = ToolCallingAgent::new(LoopLlm, tools, "mock".to_string()).max_steps(2);
    let err = agent.invoke("hi".to_string()).await.unwrap_err();
    assert!(matches!(err, WesichainError::Custom(_)));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-agent --test agent_loop -v`
Expected: FAIL with missing `ToolCallingAgent`.

**Step 3: Write minimal implementation**

```rust
// wesichain-agent/src/agent.rs
use wesichain_core::{Runnable, Value, WesichainError};
use wesichain_llm::{LlmRequest, LlmResponse, Message, Role};

use crate::ToolRegistry;

pub struct ToolCallingAgent<L> {
    llm: L,
    tools: ToolRegistry,
    model: String,
    max_steps: usize,
}

impl<L> ToolCallingAgent<L> {
    pub fn new(llm: L, tools: ToolRegistry, model: String) -> Self {
        Self { llm, tools, model, max_steps: 5 }
    }

    pub fn max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }
}

#[async_trait::async_trait]
impl<L> Runnable<String, String> for ToolCallingAgent<L>
where
    L: Runnable<LlmRequest, LlmResponse> + Send + Sync,
{
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        let mut messages = vec![Message { role: Role::User, content: input, tool_call_id: None }];

        for _ in 0..self.max_steps {
            let tool_specs = self.tools.to_specs();
            let response = self
                .llm
                .invoke(LlmRequest { model: self.model.clone(), messages: messages.clone(), tools: tool_specs })
                .await?;
            if response.tool_calls.is_empty() {
                return Ok(response.content);
            }

            for call in response.tool_calls {
                let result = self.tools.call(&call.name, call.args).await?;
                messages.push(Message {
                    role: Role::Tool,
                    content: result.to_string(),
                    tool_call_id: Some(call.id.clone()),
                });
            }
        }

        Err(WesichainError::Custom(format!("max steps exceeded: {}", self.max_steps)))
    }

    fn stream(&self, input: String) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::once(async move { Ok(wesichain_core::StreamEvent::FinalAnswer(input)) }).boxed()
    }
}
```

```rust
// wesichain-agent/src/lib.rs
mod agent;

pub use agent::ToolCallingAgent;
```

```toml
# wesichain-agent/Cargo.toml
[dependencies]
wesichain-llm = { path = "../wesichain-llm" }
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-agent --test agent_loop -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-agent/src/agent.rs wesichain-agent/src/lib.rs wesichain-agent/Cargo.toml wesichain-agent/tests/agent_loop.rs
git commit -m "feat(agent): add tool-calling agent"
```

---

### Task 9: Add agent example

**Files:**
- Create: `examples/agent.rs`
- Modify: `Cargo.toml`

**Step 1: Write the failing test**

```rust
#[test]
fn agent_example_compiles() {
    let _ = include_str!("../../examples/agent.rs");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-core --test agent_example -v`
Expected: FAIL (file not found).

**Step 3: Write minimal implementation**

```rust
// examples/agent.rs
use wesichain_agent::{Tool, ToolCallingAgent, ToolRegistry};
use wesichain_llm::OllamaClient;
use wesichain_core::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(EchoTool));

    let llm = OllamaClient::new("http://localhost:11434".to_string(), "llama3.1".to_string()).expect("client");
    let agent = ToolCallingAgent::new(llm, tools, "llama3.1".to_string());
    let result = agent.invoke("hello".to_string()).await?;
    println!("{result}");
    Ok(())
}

struct EchoTool;

#[async_trait::async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str { "echo" }
    fn description(&self) -> &str { "echo" }
    fn schema(&self) -> Value { Value::from("schema") }
    async fn call(&self, input: Value) -> Result<Value, wesichain_core::WesichainError> { Ok(input) }
}
```

```toml
# Cargo.toml
[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-core --test agent_example -v`
Expected: PASS

**Step 5: Commit**

```bash
git add examples/agent.rs Cargo.toml
git commit -m "docs: add agent example"
```

---

## Phase 2 Acceptance Criteria
- `wesichain-llm` defines request/response types and a `Llm` trait.
- Ollama client supports invoke and streaming; stream emits `StreamEvent` chunks.
- `wesichain-prompt` provides a working template renderer.
- `wesichain-agent` has Tool registry and a minimal tool-calling loop.
- Tests cover serialization, stream parsing, tool calls, and retry edges.
- Example compiles and runs against local Ollama.

## Out of Scope (Phase 2)
- Advanced memory/vector stores.
- Graph orchestration or parallel tool execution.
- Timeout/circuit breaker wrappers beyond `with_retries`.
