# Phase 5 ReAct + Tool Calling Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a graph-native ReActAgentNode with tool calling, shared LLM/tool primitives in core, OpenAI tool calling integration, and observability hooks.

**Architecture:** Move shared tool/LLM types into wesichain-core, introduce ToolCallingLlm, ReActStep, and state traits. Implement ReActAgentNode in wesichain-graph with structured messages, sequential tool execution, configurable tool failure policy, and observer events. Provide OpenAI tool calling under a feature flag in wesichain-llm.

**Tech Stack:** Rust 1.75+, async-trait, serde/serde_json, thiserror, async-openai (feature), tokio.

---

### Task 1: Move LLM + tool-call types into core

**Files:**
- Create: `wesichain-core/src/llm.rs`
- Modify: `wesichain-core/src/lib.rs`
- Modify: `wesichain-llm/src/lib.rs`
- Modify: `wesichain-llm/src/ollama.rs`
- Modify: `wesichain-llm/tests/types.rs`
- Delete: `wesichain-llm/src/types.rs`
- Test: `wesichain-core/tests/llm_types.rs`

**Step 1: Write the failing test**

```rust
use serde_json::json;
use wesichain_core::{LlmRequest, LlmResponse, Message, Role, ToolCall, ToolSpec};

#[test]
fn llm_types_serialize_with_tool_calls() {
    let call = ToolCall {
        id: "call-1".to_string(),
        name: "calculator".to_string(),
        args: json!({"expression": "2+2"}),
    };
    let message = Message {
        role: Role::Assistant,
        content: "".to_string(),
        tool_call_id: None,
        tool_calls: vec![call.clone()],
    };
    let req = LlmRequest {
        model: "test".to_string(),
        messages: vec![message],
        tools: vec![ToolSpec {
            name: "calculator".to_string(),
            description: "math".to_string(),
            parameters: json!({"type": "object"}),
        }],
    };
    let value = serde_json::to_value(req).expect("serialize request");
    assert!(value["messages"][0]["tool_calls"].is_array());

    let response = LlmResponse {
        content: "".to_string(),
        tool_calls: vec![call],
    };
    let response_value = serde_json::to_value(response).expect("serialize response");
    assert!(response_value["tool_calls"].is_array());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-core --test llm_types -v`
Expected: FAIL with missing types in core.

**Step 3: Write minimal implementation**

```rust
// wesichain-core/src/llm.rs
use serde::{Deserialize, Serialize};
use crate::Value;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LlmResponse {
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}
```

```rust
// wesichain-core/src/lib.rs
mod llm;

pub use llm::{LlmRequest, LlmResponse, Message, Role, ToolCall, ToolSpec};
```

```rust
// wesichain-llm/src/lib.rs
mod ollama;

pub use ollama::{ollama_stream_events, OllamaClient};
pub use wesichain_core::{LlmRequest, LlmResponse, Message, Role, ToolCall, ToolSpec};
```

```rust
// wesichain-llm/src/ollama.rs
use wesichain_core::{LlmRequest, LlmResponse, Message, ToolCall, ToolSpec};
```

Remove `wesichain-llm/src/types.rs` and update any remaining imports.

**Step 4: Run tests to verify they pass**

Run:
- `cargo test -p wesichain-core --test llm_types -v`
- `cargo test -p wesichain-llm --test types -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-core/src/llm.rs wesichain-core/src/lib.rs wesichain-core/tests/llm_types.rs
git add wesichain-llm/src/lib.rs wesichain-llm/src/ollama.rs wesichain-llm/tests/types.rs
git rm wesichain-llm/src/types.rs
git commit -m "feat(core): move llm tool types to core"
```

---

### Task 2: Tool + ToolError in core, update agent

**Files:**
- Create: `wesichain-core/src/tool.rs`
- Modify: `wesichain-core/src/lib.rs`
- Modify: `wesichain-agent/src/tool.rs`
- Modify: `wesichain-agent/src/agent.rs`
- Modify: `wesichain-agent/src/lib.rs`
- Test: `wesichain-core/tests/tool_error.rs`

**Step 1: Write the failing test**

```rust
use wesichain_core::{ToolError};

#[test]
fn tool_error_is_displayable() {
    let err = ToolError::InvalidInput("missing field".to_string());
    assert!(err.to_string().contains("missing field"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-core --test tool_error -v`
Expected: FAIL with missing ToolError.

**Step 3: Write minimal implementation**

```rust
// wesichain-core/src/tool.rs
use crate::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    async fn invoke(&self, args: Value) -> Result<Value, ToolError>;
}
```

```rust
// wesichain-core/src/lib.rs
mod tool;

pub use tool::{Tool, ToolError};
```

Update wesichain-agent:

```rust
// wesichain-agent/src/tool.rs
use wesichain_core::{Tool, ToolError, Value, WesichainError};

// ToolRegistry::call now returns ToolError, ToolCallingAgent maps it to WesichainError.
```

```rust
// wesichain-agent/src/agent.rs
// Map ToolError into WesichainError::ToolCallFailed
```

```rust
// wesichain-agent/src/lib.rs
pub use wesichain_core::Tool;
```

**Step 4: Run tests to verify they pass**

Run:
- `cargo test -p wesichain-core --test tool_error -v`
- `cargo test -p wesichain-agent --tests -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-core/src/tool.rs wesichain-core/src/lib.rs wesichain-core/tests/tool_error.rs
git add wesichain-agent/src/tool.rs wesichain-agent/src/agent.rs wesichain-agent/src/lib.rs
git commit -m "feat(core): add tool trait and ToolError"
```

---

### Task 3: ReActStep + state traits in core

**Files:**
- Create: `wesichain-core/src/react.rs`
- Modify: `wesichain-core/src/lib.rs`
- Test: `wesichain-core/tests/react_state.rs`

**Step 1: Write the failing test**

```rust
use std::collections::HashMap;

use serde_json::json;
use wesichain_core::{HasFinalOutput, HasUserInput, ReActStep, ScratchpadState, ToolCall, Value};

#[derive(Clone, serde::Serialize, serde::Deserialize, Default)]
struct DemoState {
    input: String,
    scratchpad: Vec<ReActStep>,
    final_output: Option<String>,
    iterations: u32,
}

impl ScratchpadState for DemoState {
    fn scratchpad(&self) -> &Vec<ReActStep> {
        &self.scratchpad
    }
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> {
        &mut self.scratchpad
    }
    fn iteration_count(&self) -> u32 {
        self.iterations
    }
    fn increment_iteration(&mut self) {
        self.iterations += 1;
    }
}

impl HasUserInput for DemoState {
    fn user_input(&self) -> &str {
        &self.input
    }
}

impl HasFinalOutput for DemoState {
    fn final_output(&self) -> Option<&str> {
        self.final_output.as_deref()
    }
    fn set_final_output(&mut self, value: String) {
        self.final_output = Some(value);
    }
}

#[test]
fn react_step_serde_roundtrip() {
    let step = ReActStep::Observation(json!({"result": 1}));
    let value = serde_json::to_value(&step).expect("serialize");
    let decoded: ReActStep = serde_json::from_value(value).expect("deserialize");
    assert!(matches!(decoded, ReActStep::Observation(_)));
}

#[test]
fn hash_map_has_user_input_impl() {
    let mut map: HashMap<String, Value> = HashMap::new();
    map.insert("input".to_string(), json!("hi"));
    assert_eq!(map.user_input(), "hi");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-core --test react_state -v`
Expected: FAIL with missing traits and ReActStep.

**Step 3: Write minimal implementation**

```rust
// wesichain-core/src/react.rs
use std::collections::HashMap;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{ToolCall, Value};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ReActStep {
    Thought(String),
    Action(ToolCall),
    Observation(Value),
    FinalAnswer(String),
    Error(String),
}

pub trait ScratchpadState: Serialize + DeserializeOwned {
    fn scratchpad(&self) -> &Vec<ReActStep>;
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep>;
    fn iteration_count(&self) -> u32;
    fn increment_iteration(&mut self);
    fn ensure_scratchpad(&mut self) {}
}

pub trait HasUserInput {
    fn user_input(&self) -> &str;
}

pub trait HasFinalOutput {
    fn final_output(&self) -> Option<&str>;
    fn set_final_output(&mut self, value: String);
}

impl HasUserInput for HashMap<String, Value> {
    fn user_input(&self) -> &str {
        self.get("input").and_then(|v| v.as_str()).unwrap_or("")
    }
}

impl HasFinalOutput for HashMap<String, Value> {
    fn final_output(&self) -> Option<&str> {
        self.get("final_output").and_then(|v| v.as_str())
    }
    fn set_final_output(&mut self, value: String) {
        self.insert("final_output".to_string(), Value::String(value));
    }
}

impl ScratchpadState for HashMap<String, Value> {
    fn scratchpad(&self) -> &Vec<ReActStep> {
        self.get("agent_scratchpad")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.get(0))
            .and_then(|_| None)
            .expect("scratchpad not initialized")
    }
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> {
        self.ensure_scratchpad();
        self.get_mut("agent_scratchpad")
            .and_then(|v| v.as_array_mut())
            .and_then(|_| None)
            .expect("scratchpad not initialized")
    }
    fn iteration_count(&self) -> u32 {
        self.get("iteration_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32
    }
    fn increment_iteration(&mut self) {
        let current = self.iteration_count() + 1;
        self.insert("iteration_count".to_string(), Value::Number(current.into()));
    }
    fn ensure_scratchpad(&mut self) {
        self.entry("agent_scratchpad".to_string())
            .or_insert_with(|| Value::Array(vec![]));
    }
}
```

Update `wesichain-core/src/lib.rs`:

```rust
mod react;

pub use react::{HasFinalOutput, HasUserInput, ReActStep, ScratchpadState};
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-core --test react_state -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-core/src/react.rs wesichain-core/src/lib.rs wesichain-core/tests/react_state.rs
git commit -m "feat(core): add react step and state traits"
```

---

### Task 4: ToolCallingLlm trait + OpenAI integration

**Files:**
- Modify: `wesichain-core/src/llm.rs`
- Modify: `wesichain-core/src/lib.rs`
- Create: `wesichain-llm/src/openai.rs`
- Modify: `wesichain-llm/src/lib.rs`
- Modify: `wesichain-llm/Cargo.toml`
- Test: `wesichain-llm/tests/openai_compile.rs`

**Step 1: Write the failing test**

```rust
#[cfg(feature = "openai")]
#[test]
fn openai_tool_calling_compiles() {
    use wesichain_llm::OpenAiClient;
    let _ = OpenAiClient::new("gpt-4o-mini".to_string());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-llm --test openai_compile -v --features openai`
Expected: FAIL with missing OpenAiClient.

**Step 3: Write minimal implementation**

```rust
// wesichain-core/src/llm.rs
#[async_trait::async_trait]
pub trait ToolCallingLlm: Send + Sync + 'static {
    async fn invoke(&self, request: LlmRequest) -> Result<LlmResponse, WesichainError>;
}
```

```rust
// wesichain-llm/src/openai.rs
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestMessageRole, ChatCompletionRequestTool,
    CreateChatCompletionRequestArgs, FunctionObject,
};
use async_openai::Client;

use wesichain_core::{LlmRequest, LlmResponse, Message, Role, ToolCall, ToolCallingLlm, ToolSpec, WesichainError};

#[derive(Clone)]
pub struct OpenAiClient {
    client: Client,
    model: String,
}

impl OpenAiClient {
    pub fn new(model: String) -> Self {
        Self { client: Client::new(), model }
    }
}

#[async_trait::async_trait]
impl ToolCallingLlm for OpenAiClient {
    async fn invoke(&self, request: LlmRequest) -> Result<LlmResponse, WesichainError> {
        let messages = request
            .messages
            .into_iter()
            .map(|message| ChatCompletionRequestMessage {
                role: match message.role {
                    Role::System => ChatCompletionRequestMessageRole::System,
                    Role::User => ChatCompletionRequestMessageRole::User,
                    Role::Assistant => ChatCompletionRequestMessageRole::Assistant,
                    Role::Tool => ChatCompletionRequestMessageRole::Tool,
                },
                content: Some(message.content),
                tool_call_id: message.tool_call_id,
                tool_calls: message.tool_calls.into_iter().map(|call| call.into()).collect(),
                ..Default::default()
            })
            .collect::<Vec<_>>();

        let tools = request.tools.into_iter().map(|tool| ChatCompletionRequestTool {
            r#type: "function".to_string(),
            function: FunctionObject {
                name: tool.name,
                description: Some(tool.description),
                parameters: Some(tool.parameters),
            },
        }).collect::<Vec<_>>();

        let response = self
            .client
            .chat()
            .create(
                CreateChatCompletionRequestArgs::default()
                    .model(&self.model)
                    .messages(messages)
                    .tools(tools)
                    .build()
                    .map_err(|err| WesichainError::LlmProvider(err.to_string()))?,
            )
            .await
            .map_err(|err| WesichainError::LlmProvider(err.to_string()))?;

        let choice = response.choices.first().ok_or_else(|| {
            WesichainError::LlmProvider("no choices returned".to_string())
        })?;
        let content = choice.message.content.clone().unwrap_or_default();
        let tool_calls = choice
            .message
            .tool_calls
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|call| ToolCall {
                id: call.id,
                name: call.function.name,
                args: call.function.arguments,
            })
            .collect();

        Ok(LlmResponse { content, tool_calls })
    }
}
```

Update `wesichain-llm/src/lib.rs`:

```rust
#[cfg(feature = "openai")]
mod openai;

#[cfg(feature = "openai")]
pub use openai::OpenAiClient;
```

Update `wesichain-llm/Cargo.toml`:

```toml
[dependencies]
async-openai = { version = "0.21", optional = true }

[features]
openai = ["async-openai"]
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-llm --test openai_compile -v --features openai`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-core/src/llm.rs wesichain-core/src/lib.rs
git add wesichain-llm/src/openai.rs wesichain-llm/src/lib.rs wesichain-llm/Cargo.toml
git add wesichain-llm/tests/openai_compile.rs
git commit -m "feat(llm): add tool-calling OpenAI client"
```

---

### Task 5: Graph observer + context

**Files:**
- Create: `wesichain-graph/src/observer.rs`
- Modify: `wesichain-graph/src/config.rs`
- Modify: `wesichain-graph/src/graph.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Test: `wesichain-graph/tests/observer.rs`

**Step 1: Write the failing test**

```rust
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, WesichainError};
use wesichain_graph::{ExecutionOptions, GraphBuilder, GraphState, Observer, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState { count: i32 }
impl StateSchema for DemoState {}

struct AddOne;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for AddOne {
    async fn invoke(&self, input: GraphState<DemoState>) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState { count: input.data.count + 1 }))
    }
    fn stream(&self, _input: GraphState<DemoState>) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[derive(Default)]
struct CollectingObserver {
    events: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl Observer for CollectingObserver {
    async fn on_node_start(&self, node_id: &str, _input: &serde_json::Value) {
        self.events.lock().unwrap().push(format!("start:{node_id}"));
    }
    async fn on_node_end(&self, node_id: &str, _output: &serde_json::Value, _duration_ms: u128) {
        self.events.lock().unwrap().push(format!("end:{node_id}"));
    }
    async fn on_error(&self, node_id: &str, _error: &wesichain_graph::GraphError) {
        self.events.lock().unwrap().push(format!("error:{node_id}"));
    }
}

#[tokio::test]
async fn observer_receives_node_events() {
    let observer = CollectingObserver::default();
    let events = observer.events.clone();
    let graph = GraphBuilder::new()
        .add_node("add", AddOne)
        .set_entry("add")
        .build();
    let options = ExecutionOptions {
        observer: Some(Arc::new(observer)),
        ..ExecutionOptions::default()
    };

    let state = GraphState::new(DemoState { count: 0 });
    let _ = graph.invoke_with_options(state, options).await.unwrap();
    let captured = events.lock().unwrap().clone();
    assert_eq!(captured, vec!["start:add", "end:add"]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test observer -v`
Expected: FAIL with missing Observer and ExecutionOptions field.

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/observer.rs
#[async_trait::async_trait]
pub trait Observer: Send + Sync + 'static {
    async fn on_node_start(&self, node_id: &str, input: &serde_json::Value);
    async fn on_node_end(&self, node_id: &str, output: &serde_json::Value, duration_ms: u128);
    async fn on_error(&self, node_id: &str, error: &crate::GraphError);
    async fn on_tool_call(&self, _node_id: &str, _tool_name: &str, _args: &serde_json::Value) {}
    async fn on_tool_result(&self, _node_id: &str, _tool_name: &str, _result: &serde_json::Value) {}
}
```

```rust
// wesichain-graph/src/config.rs
use std::sync::Arc;
use crate::Observer;

#[derive(Clone, Debug, Default)]
pub struct ExecutionOptions {
    pub max_steps: Option<usize>,
    pub cycle_detection: Option<bool>,
    pub cycle_window: Option<usize>,
    pub observer: Option<Arc<dyn Observer>>,
}
```

```rust
// wesichain-graph/src/graph.rs
// Add GraphContext and GraphNode
pub struct GraphContext {
    pub remaining_steps: Option<usize>,
    pub observer: Option<std::sync::Arc<dyn Observer>>,
}

#[async_trait::async_trait]
pub trait GraphNode<S: StateSchema>: Send + Sync {
    async fn invoke_with_context(
        &self,
        input: GraphState<S>,
        context: &GraphContext,
    ) -> Result<StateUpdate<S>, WesichainError>;
}

#[async_trait::async_trait]
impl<S, R> GraphNode<S> for R
where
    S: StateSchema,
    R: Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync,
{
    async fn invoke_with_context(
        &self,
        input: GraphState<S>,
        _context: &GraphContext,
    ) -> Result<StateUpdate<S>, WesichainError> {
        self.invoke(input).await
    }
}
```

Update GraphBuilder storage to Box<dyn GraphNode<S>> and call invoke_with_context. Build a GraphContext each loop with remaining_steps = effective.max_steps.map(|max| max.saturating_sub(step_count)). Fire observer start/end/error with serde_json::to_value state.

```rust
// wesichain-graph/src/lib.rs
mod observer;
pub use observer::Observer;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test observer -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/observer.rs wesichain-graph/src/config.rs wesichain-graph/src/graph.rs wesichain-graph/src/lib.rs
git add wesichain-graph/tests/observer.rs
git commit -m "feat(graph): add observer and execution context"
```

---

### Task 6: ReActAgentNode in graph

**Files:**
- Create: `wesichain-graph/src/react_agent.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Modify: `wesichain-graph/src/error.rs`
- Modify: `wesichain-graph/Cargo.toml`
- Test: `wesichain-graph/tests/react_agent.rs`

**Step 1: Write the failing tests**

```rust
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;
use wesichain_core::{
    HasFinalOutput, HasUserInput, LlmRequest, LlmResponse, ReActStep, ScratchpadState, Tool,
    ToolCall, ToolCallingLlm, ToolError, Value,
};
use wesichain_graph::{
    ExecutionOptions, GraphBuilder, GraphState, ReActAgentNode, StateSchema,
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    input: String,
    scratchpad: Vec<ReActStep>,
    final_output: Option<String>,
    iterations: u32,
}

impl StateSchema for DemoState {}

impl ScratchpadState for DemoState {
    fn scratchpad(&self) -> &Vec<ReActStep> { &self.scratchpad }
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> { &mut self.scratchpad }
    fn iteration_count(&self) -> u32 { self.iterations }
    fn increment_iteration(&mut self) { self.iterations += 1; }
}

impl HasUserInput for DemoState {
    fn user_input(&self) -> &str { &self.input }
}

impl HasFinalOutput for DemoState {
    fn final_output(&self) -> Option<&str> { self.final_output.as_deref() }
    fn set_final_output(&mut self, value: String) { self.final_output = Some(value); }
}

struct MockTool;

#[async_trait::async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str { "calculator" }
    fn description(&self) -> &str { "math" }
    fn schema(&self) -> Value { json!({"type": "object"}) }
    async fn invoke(&self, _args: Value) -> Result<Value, ToolError> { Ok(json!(4)) }
}

struct MockLlm;

#[async_trait::async_trait]
impl ToolCallingLlm for MockLlm {
    async fn invoke(&self, _request: LlmRequest) -> Result<LlmResponse, wesichain_core::WesichainError> {
        Ok(LlmResponse {
            content: "".to_string(),
            tool_calls: vec![ToolCall { id: "c1".to_string(), name: "calculator".to_string(), args: json!({"expression": "2+2"}) }],
        })
    }
}

#[tokio::test]
async fn react_agent_executes_tool_and_finishes() {
    let llm = Arc::new(MockLlm);
    let tool = Arc::new(MockTool);
    let node = ReActAgentNode::builder()
        .llm(llm)
        .tools(vec![tool])
        .build()
        .unwrap();

    let graph = GraphBuilder::new().add_node("agent", node).set_entry("agent").build();
    let state = GraphState::new(DemoState { input: "2+2".to_string(), ..Default::default() });
    let out = graph.invoke_with_options(state, ExecutionOptions::default()).await.unwrap();
    assert_eq!(out.data.final_output.as_deref(), Some(""));
    assert!(out.data.scratchpad.iter().any(|step| matches!(step, ReActStep::Observation(_))));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-graph --test react_agent -v`
Expected: FAIL with missing ReActAgentNode and ToolCallingLlm integration.

**Step 3: Write minimal implementation**

Implement `ReActAgentNode` with builder, prompt, tool failure policy, message building, sequential tool execution, and observer hooks. Add GraphError variants:

```rust
// wesichain-graph/src/error.rs
#[error("tool call failed for '{tool_name}': {reason}")]
ToolCallFailed { tool_name: String, reason: String },
#[error("invalid tool call response: {0}")]
InvalidToolCallResponse(String),
#[error("duplicate tool name: {0}")]
DuplicateToolName(String),
```

Add new module to lib.rs:

```rust
mod react_agent;
pub use react_agent::{ReActAgentNode, ToolFailurePolicy};
```

Add `wesichain-prompt` dependency in `wesichain-graph/Cargo.toml`.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-graph --test react_agent -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/react_agent.rs wesichain-graph/src/error.rs wesichain-graph/src/lib.rs wesichain-graph/Cargo.toml
git add wesichain-graph/tests/react_agent.rs
git commit -m "feat(graph): add react agent node"
```

---

### Task 7: Deprecate ToolCallingAgent and update README

**Files:**
- Modify: `wesichain-agent/src/agent.rs`
- Modify: `wesichain-agent/src/lib.rs`
- Modify: `README.md`

**Step 1: Update agent deprecation**

Add:

```rust
#[deprecated(since = "0.x", note = "Use ReActAgentNode in wesichain-graph")]
pub struct ToolCallingAgent<L> { /* ... */ }
```

**Step 2: Update README**

Add a minimal ReAct graph example showing:
- state struct implementing ScratchpadState, HasUserInput, HasFinalOutput
- building ReActAgentNode
- invoking graph

**Step 3: Commit**

```bash
git add wesichain-agent/src/agent.rs wesichain-agent/src/lib.rs README.md
git commit -m "docs(agent): deprecate ToolCallingAgent in favor of graph"
```
