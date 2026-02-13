# Created Users/bene/Documents/bene/python/rechain/wesichain/.worktrees/langsmith-compatibility/docs/plans/2026-02-05-wesichain-langsmith-compatibility-implementation-plan.md
# Wesichain LangSmith Compatibility Enhancement Implementation Plan
> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.
**Goal:** Implement callback-based tracing in core, instrument graphs/agents/LLMs/tools, and add a LangSmith handler that exports runs with batching, sampling, and retries.
**Architecture:** Add a `wesichain-core::callbacks` module that defines RunContext, CallbackHandler, and helpers. Instrument graph and agent entry points to emit run events via a CallbackManager. Implement `wesichain-langsmith` as a separate crate that maps callback events to LangSmith Runs API calls.
**Tech Stack:** Rust 1.75, tokio, async-trait, serde/serde_json, uuid, reqwest, wiremock, dashmap, regex, secrecy, chrono, tracing.
---
### Task 0: Baseline alignment
**Files:**
- Read: `docs/plans/2026-02-05-wesichain-langsmith-compatibility-design.md`
- Read: `wesichain-core/src/lib.rs`
- Read: `wesichain-graph/src/graph.rs`
- Read: `wesichain-agent/src/agent.rs`
**Step 1: Verify baseline tests**
Run: `cargo test`
Expected: PASS (baseline before changes)
**Step 2: Confirm run_type mapping decision**
Decision: Use `chain` for graph nodes and `graph` for root if LangSmith UI renders it; otherwise use `chain` for root. Document the decision in code comments where the root run is created.
**Step 3: Commit**
No commit for this task.
---
### Task 1: Add core callbacks module
**Files:**
- Create: `wesichain-core/src/callbacks/mod.rs`
- Modify: `wesichain-core/src/lib.rs`
- Modify: `wesichain-core/Cargo.toml`
- Create: `wesichain-core/tests/callbacks.rs`
**Step 1: Write the failing tests**
Create `wesichain-core/tests/callbacks.rs`:
```rust
use std::collections::BTreeMap;
use wesichain_core::callbacks::{ensure_object, CallbackManager, RunContext, RunType};
use wesichain_core::Value;
#[test]
fn child_context_inherits_trace_and_parent() {
    let root = RunContext::root(RunType::Graph, "graph".to_string(), vec![], BTreeMap::new());
    let child = root.child(RunType::Chain, "node".to_string());
    assert_eq!(child.parent_run_id, Some(root.run_id));
    assert_eq!(child.trace_id, root.trace_id);
}
#[test]
fn ensure_object_wraps_primitives() {
    let value = Value::String("hello".to_string());
    let wrapped = ensure_object(value);
    assert!(wrapped.is_object());
}
#[test]
fn callback_manager_noop_has_no_handlers() {
    let manager = CallbackManager::noop();
    assert!(manager.is_noop());
}
```
**Step 2: Run tests to verify failure**
Run: `cargo test -p wesichain-core callbacks -v`
Expected: FAIL (callbacks module missing)
**Step 3: Implement callbacks module**
Create `wesichain-core/src/callbacks/mod.rs`:
```rust
use std::collections::BTreeMap;
use std::time::{Instant, SystemTime};
use async_trait::async_trait;
use serde::Serialize;
use uuid::Uuid;
use crate::Value;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunType {
    Chain,
    Llm,
    Tool,
    Graph,
    Agent,
    Retriever,
    Runnable,
}
#[derive(Clone, Debug)]
pub struct RunContext {
    pub run_id: Uuid,
    pub parent_run_id: Option<Uuid>,
    pub trace_id: Uuid,
    pub run_type: RunType,
    pub name: String,
    pub start_time: SystemTime,
    pub start_instant: Instant,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, Value>,
}
impl RunContext {
    pub fn root(
        run_type: RunType,
        name: String,
        tags: Vec<String>,
        metadata: BTreeMap<String, Value>,
    ) -> Self {
        let run_id = Uuid::new_v4();
        Self {
            run_id,
            parent_run_id: None,
            trace_id: run_id,
            run_type,
            name,
            start_time: SystemTime::now(),
            start_instant: Instant::now(),
            tags,
            metadata,
        }
    }
    pub fn child(&self, run_type: RunType, name: String) -> Self {
        let run_id = Uuid::new_v4();
        Self {
            run_id,
            parent_run_id: Some(self.run_id),
            trace_id: self.trace_id,
            run_type,
            name,
            start_time: SystemTime::now(),
            start_instant: Instant::now(),
            tags: self.tags.clone(),
            metadata: self.metadata.clone(),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct RunConfig {
    pub callbacks: Option<CallbackManager>,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, Value>,
    pub name_override: Option<String>,
}
#[async_trait]
pub trait CallbackHandler: Send + Sync {
    async fn on_start(&self, ctx: &RunContext, inputs: &Value);
    async fn on_end(&self, ctx: &RunContext, outputs: &Value, duration_ms: u128);
    async fn on_error(&self, ctx: &RunContext, error: &Value, duration_ms: u128);
    async fn on_stream_chunk(&self, _ctx: &RunContext, _chunk: &Value) {}
}
#[derive(Clone, Default)]
pub struct CallbackManager {
    handlers: Vec<std::sync::Arc<dyn CallbackHandler>>,
}
impl CallbackManager {
    pub fn new(handlers: Vec<std::sync::Arc<dyn CallbackHandler>>) -> Self {
        Self { handlers }
    }
    pub fn noop() -> Self {
        Self { handlers: vec![] }
    }
    pub fn is_noop(&self) -> bool {
        self.handlers.is_empty()
    }
    pub async fn on_start(&self, ctx: &RunContext, inputs: &Value) {
        for handler in &self.handlers {
            handler.on_start(ctx, inputs).await;
        }
    }
    pub async fn on_end(&self, ctx: &RunContext, outputs: &Value, duration_ms: u128) {
        for handler in &self.handlers {
            handler.on_end(ctx, outputs, duration_ms).await;
        }
    }
    pub async fn on_error(&self, ctx: &RunContext, error: &Value, duration_ms: u128) {
        for handler in &self.handlers {
            handler.on_error(ctx, error, duration_ms).await;
        }
    }
    pub async fn on_stream_chunk(&self, ctx: &RunContext, chunk: &Value) {
        for handler in &self.handlers {
            handler.on_stream_chunk(ctx, chunk).await;
        }
    }
}
pub trait ToTraceInput {
    fn to_trace_input(&self) -> Value;
}
pub trait ToTraceOutput {
    fn to_trace_output(&self) -> Value;
}
impl<T> ToTraceInput for T
where
    T: Serialize,
{
    fn to_trace_input(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}
impl<T> ToTraceOutput for T
where
    T: Serialize,
{
    fn to_trace_output(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}
pub fn ensure_object(value: Value) -> Value {
    match value {
        Value::Object(_) => value,
        other => Value::Object(serde_json::Map::from_iter([(
            "value".to_string(),
            other,
        )])),
    }
}
```
Update `wesichain-core/src/lib.rs`:
```rust
mod callbacks;
pub use callbacks::{
    ensure_object, CallbackHandler, CallbackManager, RunConfig, RunContext, RunType, ToTraceInput,
    ToTraceOutput,
};
```
Update `wesichain-core/Cargo.toml` (add dependency):
```toml
uuid = { version = "1", features = ["v4", "serde"] }
```
**Step 4: Run tests to verify pass**
Run: `cargo test -p wesichain-core callbacks -v`
Expected: PASS
**Step 5: Commit**
```bash
git add wesichain-core/src/callbacks/mod.rs wesichain-core/src/lib.rs wesichain-core/Cargo.toml wesichain-core/tests/callbacks.rs
git commit -m "feat(core): add callbacks module"
```
---
### Task 2: Add traced runnable wrapper
**Files:**
- Create: `wesichain-core/src/callbacks/wrappers.rs`
- Modify: `wesichain-core/src/callbacks/mod.rs`
- Create: `wesichain-core/tests/traced_runnable.rs`
**Step 1: Write the failing test**
Create `wesichain-core/tests/traced_runnable.rs`:
```rust
use std::sync::{Arc, Mutex};
use wesichain_core::callbacks::{CallbackHandler, CallbackManager, RunContext, RunType, TracedRunnable};
use wesichain_core::{Runnable, Value, WesichainError};
struct RecordingHandler {
    events: Arc<Mutex<Vec<String>>>,
}
#[async_trait::async_trait]
impl CallbackHandler for RecordingHandler {
    async fn on_start(&self, _ctx: &RunContext, _inputs: &Value) {
        self.events.lock().unwrap().push("start".to_string());
    }
    async fn on_end(&self, _ctx: &RunContext, _outputs: &Value, _duration_ms: u128) {
        self.events.lock().unwrap().push("end".to_string());
    }
    async fn on_error(&self, _ctx: &RunContext, _error: &Value, _duration_ms: u128) {
        self.events.lock().unwrap().push("error".to_string());
    }
}
struct OkRunnable;
#[async_trait::async_trait]
impl Runnable<String, String> for OkRunnable {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(format!("{}!", input))
    }
    fn stream(
        &self,
        _input: String,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}
#[tokio::test]
async fn traced_runnable_emits_start_and_end() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let handler = Arc::new(RecordingHandler { events: events.clone() });
    let manager = CallbackManager::new(vec![handler]);
    let root = RunContext::root(RunType::Chain, "root".to_string(), vec![], Default::default());
    let traced = TracedRunnable::new(OkRunnable, manager, root, RunType::Chain, "node".to_string());
    let _ = traced.invoke("hi".to_string()).await.unwrap();
    let events = events.lock().unwrap().clone();
    assert_eq!(events, vec!["start", "end"]);
}
```
**Step 2: Run tests to verify failure**
Run: `cargo test -p wesichain-core traced_runnable -v`
Expected: FAIL (wrapper missing)
**Step 3: Implement wrapper**
Create `wesichain-core/src/callbacks/wrappers.rs`:
```rust
use std::time::Duration;
use crate::callbacks::{ensure_object, CallbackManager, RunContext, RunType, ToTraceInput, ToTraceOutput};
use crate::{Runnable, StreamEvent, WesichainError};
pub struct TracedRunnable<R> {
    inner: R,
    manager: CallbackManager,
    parent: RunContext,
    run_type: RunType,
    name: String,
}
impl<R> TracedRunnable<R> {
    pub fn new(inner: R, manager: CallbackManager, parent: RunContext, run_type: RunType, name: String) -> Self {
        Self {
            inner,
            manager,
            parent,
            run_type,
            name,
        }
    }
}
#[async_trait::async_trait]
impl<Input, Output, R> Runnable<Input, Output> for TracedRunnable<R>
where
    Input: Send + Sync + 'static,
    Output: Send + Sync + 'static,
    R: Runnable<Input, Output> + Send + Sync,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        if self.manager.is_noop() {
            return self.inner.invoke(input).await;
        }
        let ctx = self.parent.child(self.run_type.clone(), self.name.clone());
        let inputs = ensure_object(input.to_trace_input());
        self.manager.on_start(&ctx, &inputs).await;
        let result = self.inner.invoke(input).await;
        let duration = ctx.start_instant.elapsed().as_millis();
        match &result {
            Ok(output) => {
                let outputs = ensure_object(output.to_trace_output());
                self.manager.on_end(&ctx, &outputs, duration).await;
            }
            Err(err) => {
                let error = ensure_object(err.to_string().to_trace_output());
                self.manager.on_error(&ctx, &error, duration).await;
            }
        }
        result
    }
    fn stream(
        &self,
        input: Input,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        self.inner.stream(input)
    }
}
```
Update `wesichain-core/src/callbacks/mod.rs` to export `TracedRunnable`:
```rust
mod wrappers;
pub use wrappers::TracedRunnable;
```
**Step 4: Run tests to verify pass**
Run: `cargo test -p wesichain-core traced_runnable -v`
Expected: PASS
**Step 5: Commit**
```bash
git add wesichain-core/src/callbacks/wrappers.rs wesichain-core/src/callbacks/mod.rs wesichain-core/tests/traced_runnable.rs
git commit -m "feat(core): add traced runnable"
```
---
### Task 3: Add callbacks to graph execution
**Files:**
- Modify: `wesichain-graph/src/config.rs`
- Modify: `wesichain-graph/src/graph.rs`
- Create: `wesichain-graph/tests/callbacks.rs`
**Step 1: Write the failing test**
Create `wesichain-graph/tests/callbacks.rs`:
```rust
use std::sync::{Arc, Mutex};
use wesichain_core::callbacks::{CallbackHandler, CallbackManager, RunConfig, RunContext, RunType};
use wesichain_core::{Runnable, Value, WesichainError};
use wesichain_graph::{ExecutionOptions, GraphBuilder, GraphState, StateSchema, StateUpdate};
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct DemoState {
    value: usize,
}
impl StateSchema for DemoState {}
struct IncrNode;
#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for IncrNode {
    async fn invoke(
        &self,
        input: GraphState<DemoState>,
    ) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState { value: input.data.value + 1 }))
    }
    fn stream(
        &self,
        _input: GraphState<DemoState>,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}
struct RecordingHandler {
    events: Arc<Mutex<Vec<String>>>,
}
#[async_trait::async_trait]
impl CallbackHandler for RecordingHandler {
    async fn on_start(&self, _ctx: &RunContext, _inputs: &Value) {
        self.events.lock().unwrap().push("start".to_string());
    }
    async fn on_end(&self, _ctx: &RunContext, _outputs: &Value, _duration_ms: u128) {
        self.events.lock().unwrap().push("end".to_string());
    }
    async fn on_error(&self, _ctx: &RunContext, _error: &Value, _duration_ms: u128) {}
}
#[tokio::test]
async fn graph_invocation_emits_callbacks() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let handler = Arc::new(RecordingHandler { events: events.clone() });
    let callbacks = CallbackManager::new(vec![handler]);
    let options = ExecutionOptions {
        run_config: Some(RunConfig {
            callbacks: Some(callbacks),
            ..Default::default()
        }),
        ..Default::default()
    };
    let graph = GraphBuilder::new()
        .add_node("node", IncrNode)
        .set_entry("node")
        .build();
    let _ = graph.invoke_with_options(GraphState::new(DemoState::default()), options).await.unwrap();
    let events = events.lock().unwrap().clone();
    assert!(events.len() >= 2);
}
```
**Step 2: Run tests to verify failure**
Run: `cargo test -p wesichain-graph callbacks -v`
Expected: FAIL (run_config/callbacks missing)
**Step 3: Implement callbacks in graph**
Update `wesichain-graph/src/config.rs`:
```rust
use wesichain_core::callbacks::RunConfig;
#[derive(Clone, Debug, Default)]
pub struct ExecutionOptions {
    pub max_steps: Option<usize>,
    pub cycle_detection: Option<bool>,
    pub cycle_window: Option<usize>,
    pub run_config: Option<RunConfig>,
}
```
Update `wesichain-graph/src/graph.rs` (in `invoke_with_options`):
- Create a root `RunContext` when `run_config.callbacks` is present.
- Emit `on_start` for the root with the initial state.
- Emit `on_start` and `on_end` around each node invocation.
- Emit `on_error` on failures before returning.
Use `ensure_object` and `ToTraceInput/ToTraceOutput` to serialize inputs/outputs.
**Step 4: Run tests to verify pass**
Run: `cargo test -p wesichain-graph callbacks -v`
Expected: PASS
**Step 5: Commit**
```bash
git add wesichain-graph/src/config.rs wesichain-graph/src/graph.rs wesichain-graph/tests/callbacks.rs
git commit -m "feat(graph): add callback instrumentation"
```
---
### Task 4: Add callbacks to agent and tool calls
**Files:**
- Modify: `wesichain-agent/src/agent.rs`
- Modify: `wesichain-agent/src/lib.rs`
- Create: `wesichain-agent/tests/callbacks.rs`
**Step 1: Write the failing test**
Create `wesichain-agent/tests/callbacks.rs`:
```rust
use std::sync::{Arc, Mutex};
use wesichain_agent::{Tool, ToolCallingAgent, ToolRegistry};
use wesichain_core::callbacks::{CallbackHandler, CallbackManager, RunConfig, RunContext, RunType};
use wesichain_core::{Runnable, Value, WesichainError};
use wesichain_llm::{LlmRequest, LlmResponse};
struct RecordingHandler {
    events: Arc<Mutex<Vec<String>>>,
}
#[async_trait::async_trait]
impl CallbackHandler for RecordingHandler {
    async fn on_start(&self, ctx: &RunContext, _inputs: &Value) {
        self.events.lock().unwrap().push(format!("start:{:?}", ctx.run_type));
    }
    async fn on_end(&self, ctx: &RunContext, _outputs: &Value, _duration_ms: u128) {
        self.events.lock().unwrap().push(format!("end:{:?}", ctx.run_type));
    }
    async fn on_error(&self, _ctx: &RunContext, _error: &Value, _duration_ms: u128) {}
}
struct MockLlm;
#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockLlm {
    async fn invoke(&self, _input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        Ok(LlmResponse { content: "done".to_string(), tool_calls: vec![] })
    }
    fn stream(
        &self,
        _input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}
struct MockTool;
#[async_trait::async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        "mock"
    }
    fn description(&self) -> &str {
        "mock"
    }
    fn schema(&self) -> Value {
        Value::Null
    }
    async fn call(&self, _input: Value) -> Result<Value, WesichainError> {
        Ok(Value::String("ok".to_string()))
    }
}
#[tokio::test]
async fn agent_emits_callbacks_for_llm_and_tool() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let handler = Arc::new(RecordingHandler { events: events.clone() });
    let callbacks = CallbackManager::new(vec![handler]);
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MockTool));
    let agent = ToolCallingAgent::new(MockLlm, registry, "model".to_string())
        .with_run_config(RunConfig {
            callbacks: Some(callbacks),
            ..Default::default()
        });
    let _ = agent.invoke("input".to_string()).await.unwrap();
    let events = events.lock().unwrap().clone();
    assert!(events.iter().any(|e| e.contains("Llm")));
}
```
**Step 2: Run tests to verify failure**
Run: `cargo test -p wesichain-agent callbacks -v`
Expected: FAIL (run_config missing)
**Step 3: Implement callbacks in agent**
Update `wesichain-agent/src/agent.rs`:
- Add `run_config: Option<RunConfig>` to `ToolCallingAgent`.
- Add builder method `with_run_config`.
- In `invoke`, when callbacks present:
  - Create root context `RunType::Agent` with name override if provided.
  - Emit `on_start` with user input.
  - Wrap LLM invoke with child `RunType::Llm` context and callbacks.
  - Wrap each tool call with child `RunType::Tool` context and callbacks.
  - Emit `on_end` or `on_error` for root.
Update `wesichain-agent/src/lib.rs` if new types are re-exported.
**Step 4: Run tests to verify pass**
Run: `cargo test -p wesichain-agent callbacks -v`
Expected: PASS
**Step 5: Commit**
```bash
git add wesichain-agent/src/agent.rs wesichain-agent/src/lib.rs wesichain-agent/tests/callbacks.rs
git commit -m "feat(agent): add callback instrumentation"
```
---
### Task 5: Create wesichain-langsmith crate scaffold
**Files:**
- Modify: `Cargo.toml`
- Create: `wesichain-langsmith/Cargo.toml`
- Create: `wesichain-langsmith/src/lib.rs`
**Step 1: Add workspace member**
Update `Cargo.toml`:
```toml
members = [
  "wesichain",
  "wesichain-core",
  "wesichain-prompt",
  "wesichain-llm",
  "wesichain-agent",
  "wesichain-graph",
  "wesichain-langsmith",
]
```
**Step 2: Create crate manifest**
Create `wesichain-langsmith/Cargo.toml`:
```toml
[package]
name = "wesichain-langsmith"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
repository.workspace = true
homepage.workspace = true
description = "LangSmith observability for Wesichain"
[dependencies]
async-trait = "0.1"
chrono = { version = "0.4", features = ["clock"] }
dashmap = "6"
regex = "1"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
secrecy = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync", "time"] }
tracing = "0.1"
uuid = { version = "1", features = ["v4", "serde"] }
wesichain-core = { path = "../wesichain-core" }
[dev-dependencies]
wiremock = "0.6"
wesichain-graph = { path = "../wesichain-graph" }
wesichain-agent = { path = "../wesichain-agent" }
wesichain-llm = { path = "../wesichain-llm" }
```
**Step 3: Create lib.rs with module stubs**
Create `wesichain-langsmith/src/lib.rs`:
```rust
mod client;
mod config;
mod events;
mod exporter;
mod handler;
mod run_store;
mod sampler;
mod sanitize;
pub use client::{LangSmithClient, LangSmithError};
pub use config::LangSmithConfig;
pub use events::{RunEvent, RunStatus, RunType};
pub use exporter::{FlushError, FlushStats, LangSmithExporter};
pub use handler::LangSmithCallbackHandler;
pub use run_store::{RunContextStore, RunMetadata, RunUpdateDecision};
pub use sampler::{ProbabilitySampler, Sampler};
pub use sanitize::{ensure_object, sanitize_value, truncate_value};
```
**Step 4: Verify compile**
Run: `cargo test -p wesichain-langsmith`
Expected: FAIL (modules missing)
**Step 5: Commit**
```bash
git add Cargo.toml wesichain-langsmith/Cargo.toml wesichain-langsmith/src/lib.rs
git commit -m "feat(langsmith): add crate scaffold"
```
---
### Task 6: Config, sampler, and sanitize helpers
**Files:**
- Create: `wesichain-langsmith/src/config.rs`
- Create: `wesichain-langsmith/src/sampler.rs`
- Create: `wesichain-langsmith/src/sanitize.rs`
- Create: `wesichain-langsmith/tests/sanitize.rs`
- Create: `wesichain-langsmith/tests/sampler.rs`
**Step 1: Write failing tests**
Create `wesichain-langsmith/tests/sanitize.rs`:
```rust
use regex::Regex;
use serde_json::json;
use wesichain_langsmith::{ensure_object, sanitize_value, truncate_value};
#[test]
fn redaction_applies_before_truncation() {
    let regex = Regex::new("secret").unwrap();
    let value = json!({"token": "secret-secret-secret"});
    let redacted = sanitize_value(value, Some(&regex));
    let truncated = truncate_value(redacted, 10);
    assert_eq!(truncated, json!({"token": "[REDACTED]"}));
}
#[test]
fn non_object_inputs_are_wrapped() {
    let wrapped = ensure_object(json!("hello"));
    assert!(wrapped.is_object());
}
```
Create `wesichain-langsmith/tests/sampler.rs`:
```rust
use uuid::Uuid;
use wesichain_langsmith::{ProbabilitySampler, Sampler};
#[test]
fn sampler_is_deterministic_by_run_id() {
    let sampler = ProbabilitySampler { rate: 0.5 };
    let run_id = Uuid::new_v4();
    let first = sampler.should_sample(run_id);
    let second = sampler.should_sample(run_id);
    assert_eq!(first, second);
}
#[test]
fn sampler_respects_bounds() {
    let sampler = ProbabilitySampler { rate: 0.0 };
    assert!(!sampler.should_sample(Uuid::new_v4()));
    let sampler = ProbabilitySampler { rate: 1.0 };
    assert!(sampler.should_sample(Uuid::new_v4()));
}
```
**Step 2: Run tests to verify failure**
Run: `cargo test -p wesichain-langsmith sanitize sampler -v`
Expected: FAIL (modules missing)
**Step 3: Implement helpers**
Create `wesichain-langsmith/src/config.rs`:
```rust
use std::time::Duration;
use regex::Regex;
use secrecy::SecretString;
#[derive(Clone, Debug)]
pub struct LangSmithConfig {
    pub api_key: SecretString,
    pub api_url: String,
    pub project_name: String,
    pub flush_interval: Duration,
    pub max_batch_size: usize,
    pub queue_capacity: usize,
    pub sampling_rate: f64,
    pub redact_regex: Option<Regex>,
}
impl LangSmithConfig {
    pub fn new(api_key: SecretString, project_name: impl Into<String>) -> Self {
        Self {
            api_key,
            api_url: "https://api.smith.service".to_string(),
            project_name: project_name.into(),
            flush_interval: Duration::from_secs(2),
            max_batch_size: 50,
            queue_capacity: 1000,
            sampling_rate: 1.0,
            redact_regex: None,
        }
    }
}
```
Create `wesichain-langsmith/src/sampler.rs`:
```rust
use uuid::Uuid;
pub trait Sampler: Send + Sync {
    fn should_sample(&self, run_id: Uuid) -> bool;
}
#[derive(Clone, Debug)]
pub struct ProbabilitySampler {
    pub rate: f64,
}
impl Sampler for ProbabilitySampler {
    fn should_sample(&self, run_id: Uuid) -> bool {
        if self.rate <= 0.0 {
            return false;
        }
        if self.rate >= 1.0 {
            return true;
        }
        let bytes = run_id.as_bytes();
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[0..8]);
        let hash = u64::from_le_bytes(buf);
        let ratio = (hash as f64) / (u64::MAX as f64);
        ratio < self.rate
    }
}
```
Create `wesichain-langsmith/src/sanitize.rs`:
```rust
use regex::Regex;
use serde_json::Value;
const REDACTED: &str = "[REDACTED]";
pub fn ensure_object(value: Value) -> Value {
    match value {
        Value::Object(_) => value,
        other => Value::Object(serde_json::Map::from_iter([(
            "value".to_string(),
            other,
        )])),
    }
}
pub fn sanitize_value(value: Value, regex: Option<&Regex>) -> Value {
    match value {
        Value::String(text) => match regex {
            Some(pattern) => Value::String(pattern.replace_all(&text, REDACTED).to_string()),
            None => Value::String(text),
        },
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| sanitize_value(item, regex))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, sanitize_value(value, regex)))
                .collect(),
        ),
        other => other,
    }
}
pub fn truncate_value(value: Value, max_bytes: usize) -> Value {
    match value {
        Value::String(text) => Value::String(truncate_string(&text, max_bytes)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| truncate_value(item, max_bytes))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, truncate_value(value, max_bytes)))
                .collect(),
        ),
        other => other,
    }
}
fn truncate_string(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = 0;
    for (idx, _) in text.char_indices() {
        if idx <= max_bytes {
            end = idx;
        } else {
            break;
        }
    }
    text[..end].to_string()
}
```
**Step 4: Run tests to verify pass**
Run: `cargo test -p wesichain-langsmith sanitize sampler -v`
Expected: PASS
**Step 5: Commit**
```bash
git add wesichain-langsmith/src/config.rs wesichain-langsmith/src/sampler.rs wesichain-langsmith/src/sanitize.rs wesichain-langsmith/tests/sanitize.rs wesichain-langsmith/tests/sampler.rs
git commit -m "feat(langsmith): add config, sampler, and sanitization"
```
---
### Task 7: Run events and context store
**Files:**
- Create: `wesichain-langsmith/src/events.rs`
- Create: `wesichain-langsmith/src/run_store.rs`
- Create: `wesichain-langsmith/tests/run_store.rs`
**Step 1: Write failing tests**
Create `wesichain-langsmith/tests/run_store.rs`:
```rust
use uuid::Uuid;
use wesichain_langsmith::{RunContextStore, RunStatus};
#[test]
fn first_terminal_event_is_authoritative() {
    let store = RunContextStore::default();
    let run_id = Uuid::new_v4();
    store.record_start(run_id, None);
    let first = store.apply_update(run_id, Some("boom".to_string()));
    let second = store.apply_update(run_id, None);
    assert_eq!(first.status, RunStatus::Failed);
    assert_eq!(second.status, RunStatus::Failed);
    assert_eq!(second.error.as_deref(), Some("boom"));
}
```
**Step 2: Run tests to verify failure**
Run: `cargo test -p wesichain-langsmith run_store -v`
Expected: FAIL
**Step 3: Implement run events and store**
Create `wesichain-langsmith/src/events.rs`:
```rust
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunType {
    Chain,
    Tool,
    Llm,
    Agent,
    Graph,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
}
#[derive(Clone, Debug)]
pub enum RunEvent {
    Start {
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        trace_id: Uuid,
        name: String,
        run_type: RunType,
        start_time: DateTime<Utc>,
        inputs: Value,
        tags: Vec<String>,
        metadata: Value,
        session_name: String,
    },
    Update {
        run_id: Uuid,
        end_time: Option<DateTime<Utc>>,
        outputs: Option<Value>,
        error: Option<String>,
        duration_ms: Option<u128>,
    },
}
```
Create `wesichain-langsmith/src/run_store.rs`:
```rust
use dashmap::DashMap;
use uuid::Uuid;
use crate::events::RunStatus;
#[derive(Clone, Debug)]
pub struct RunMetadata {
    pub status: RunStatus,
    pub error: Option<String>,
    pub parent_id: Option<Uuid>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunUpdateDecision {
    pub status: RunStatus,
    pub error: Option<String>,
}
#[derive(Default)]
pub struct RunContextStore {
    runs: DashMap<Uuid, RunMetadata>,
}
impl RunContextStore {
    pub fn record_start(&self, run_id: Uuid, parent_id: Option<Uuid>) {
        self.runs.insert(
            run_id,
            RunMetadata {
                status: RunStatus::Running,
                error: None,
                parent_id,
            },
        );
    }
    pub fn apply_update(&self, run_id: Uuid, error: Option<String>) -> RunUpdateDecision {
        let mut entry = self.runs.entry(run_id).or_insert(RunMetadata {
            status: RunStatus::Running,
            error: None,
            parent_id: None,
        });
        match (&entry.status, error) {
            (RunStatus::Running, Some(err)) => {
                entry.status = RunStatus::Failed;
                entry.error = Some(err.clone());
                RunUpdateDecision {
                    status: RunStatus::Failed,
                    error: Some(err),
                }
            }
            (RunStatus::Running, None) => {
                entry.status = RunStatus::Completed;
                RunUpdateDecision {
                    status: RunStatus::Completed,
                    error: None,
                }
            }
            (RunStatus::Failed, _) => RunUpdateDecision {
                status: RunStatus::Failed,
                error: entry.error.clone(),
            },
            (RunStatus::Completed, _) => RunUpdateDecision {
                status: RunStatus::Completed,
                error: None,
            },
        }
    }
}
```
**Step 4: Run tests to verify pass**
Run: `cargo test -p wesichain-langsmith run_store -v`
Expected: PASS
**Step 5: Commit**
```bash
git add wesichain-langsmith/src/events.rs wesichain-langsmith/src/run_store.rs wesichain-langsmith/tests/run_store.rs
git commit -m "feat(langsmith): add run events and context store"
```
---
### Task 8: LangSmith HTTP client
**Files:**
- Create: `wesichain-langsmith/src/client.rs`
- Create: `wesichain-langsmith/tests/client.rs`
**Step 1: Write failing tests**
Create `wesichain-langsmith/tests/client.rs`:
```rust
use secrecy::SecretString;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wesichain_langsmith::LangSmithClient;
#[tokio::test]
async fn create_run_includes_idempotency_key() {
    let server = MockServer::start().await;
    let run_id = Uuid::new_v4();
    let payload = json!({"id": run_id, "name": "demo"});
    Mock::given(method("POST"))
        .and(path("/runs"))
        .and(header("x-idempotency-key", run_id.to_string()))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let client = LangSmithClient::new(server.uri(), SecretString::new("test-key".to_string()));
    client.create_run(run_id, &payload).await.unwrap();
}
```
**Step 2: Run tests to verify failure**
Run: `cargo test -p wesichain-langsmith client -v`
Expected: FAIL
**Step 3: Implement client**
Create `wesichain-langsmith/src/client.rs`:
```rust
use std::time::Duration;
use reqwest::{header::HeaderMap, Client, Method, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use thiserror::Error;
use tokio::time::sleep;
use uuid::Uuid;
#[derive(Debug, Error)]
pub enum LangSmithError {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("http error: {status}")]
    Http { status: StatusCode, body: String },
}
#[derive(Clone)]
pub struct LangSmithClient {
    client: Client,
    api_url: String,
    api_key: SecretString,
}
impl LangSmithClient {
    pub fn new(api_url: String, api_key: SecretString) -> Self {
        Self {
            client: Client::new(),
            api_url,
            api_key,
        }
    }
    pub async fn create_run(&self, run_id: Uuid, payload: &Value) -> Result<(), LangSmithError> {
        let url = format!("{}/runs", self.api_url.trim_end_matches('/'));
        self.send_with_retry(Method::POST, &url, Some(run_id.to_string()), payload, false)
            .await
    }
    pub async fn update_run(&self, run_id: Uuid, payload: &Value) -> Result<(), LangSmithError> {
        let url = format!("{}/runs/{}", self.api_url.trim_end_matches('/'), run_id);
        self.send_with_retry(Method::PATCH, &url, None, payload, true)
            .await
    }
    async fn send_with_retry(
        &self,
        method: Method,
        url: &str,
        idempotency_key: Option<String>,
        payload: &Value,
        allow_not_found: bool,
    ) -> Result<(), LangSmithError> {
        let mut attempt = 0;
        let mut backoff = Duration::from_millis(200);
        loop {
            attempt += 1;
            let mut request = self
                .client
                .request(method.clone(), url)
                .header("x-api-key", self.api_key.expose_secret())
                .json(payload);
            if let Some(key) = &idempotency_key {
                request = request.header("x-idempotency-key", key);
            }
            match request.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(());
                    }
                    if allow_not_found && response.status() == StatusCode::NOT_FOUND {
                        return Ok(());
                    }
                    if should_retry(response.status()) && attempt < 3 {
                        backoff = next_delay(response.status(), response.headers(), backoff);
                        sleep(backoff).await;
                        continue;
                    }
                    let body = response.text().await.unwrap_or_default();
                    return Err(LangSmithError::Http {
                        status: response.status(),
                        body,
                    });
                }
                Err(err) => {
                    if (err.is_timeout() || err.is_connect()) && attempt < 3 {
                        sleep(backoff).await;
                        backoff = backoff.saturating_mul(2);
                        continue;
                    }
                    return Err(LangSmithError::Request(err));
                }
            }
        }
    }
}
fn should_retry(status: StatusCode) -> bool {
    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
}
fn next_delay(status: StatusCode, headers: &HeaderMap, backoff: Duration) -> Duration {
    if status == StatusCode::TOO_MANY_REQUESTS {
        if let Some(value) = headers.get("Retry-After").and_then(|v| v.to_str().ok()) {
            if let Ok(seconds) = value.parse::<u64>() {
                return Duration::from_secs(seconds);
            }
        }
    }
    backoff.saturating_mul(2)
}
```
**Step 4: Run tests to verify pass**
Run: `cargo test -p wesichain-langsmith client -v`
Expected: PASS
**Step 5: Commit**
```bash
git add wesichain-langsmith/src/client.rs wesichain-langsmith/tests/client.rs
git commit -m "feat(langsmith): add LangSmith HTTP client"
```
---
### Task 9: Exporter with batching and backpressure
**Files:**
- Create: `wesichain-langsmith/src/exporter.rs`
- Create: `wesichain-langsmith/tests/exporter.rs`
**Step 1: Write failing tests**
Create `wesichain-langsmith/tests/exporter.rs`:
```rust
use std::sync::Arc;
use std::time::Duration;
use chrono::Utc;
use secrecy::SecretString;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wesichain_langsmith::{
    LangSmithConfig, LangSmithExporter, RunContextStore, RunEvent, RunType,
};
#[tokio::test]
async fn drops_oldest_when_queue_full() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/runs"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let config = LangSmithConfig {
        api_key: SecretString::new("key".to_string()),
        api_url: server.uri(),
        project_name: "test".to_string(),
        flush_interval: Duration::from_secs(3600),
        max_batch_size: 10,
        queue_capacity: 1,
        sampling_rate: 1.0,
        redact_regex: None,
    };
    let exporter = LangSmithExporter::new(config, Arc::new(RunContextStore::default()));
    exporter
        .enqueue(RunEvent::Start {
            run_id: Uuid::new_v4(),
            parent_run_id: None,
            trace_id: Uuid::new_v4(),
            name: "a".to_string(),
            run_type: RunType::Chain,
            start_time: Utc::now(),
            inputs: json!({}),
            tags: vec![],
            metadata: json!({}),
            session_name: "test".to_string(),
        })
        .await;
    exporter
        .enqueue(RunEvent::Start {
            run_id: Uuid::new_v4(),
            parent_run_id: None,
            trace_id: Uuid::new_v4(),
            name: "b".to_string(),
            run_type: RunType::Chain,
            start_time: Utc::now(),
            inputs: json!({}),
            tags: vec![],
            metadata: json!({}),
            session_name: "test".to_string(),
        })
        .await;
    assert_eq!(exporter.dropped_events(), 1);
}
```
**Step 2: Run tests to verify failure**
Run: `cargo test -p wesichain-langsmith exporter -v`
Expected: FAIL
**Step 3: Implement exporter**
Create `wesichain-langsmith/src/exporter.rs` (batching, drop-oldest, async flush). Use the design doc as reference for RunEvent handling and terminal semantics, and call `LangSmithClient::create_run` / `update_run` accordingly.
**Step 4: Run tests to verify pass**
Run: `cargo test -p wesichain-langsmith exporter -v`
Expected: PASS
**Step 5: Commit**
```bash
git add wesichain-langsmith/src/exporter.rs wesichain-langsmith/tests/exporter.rs
git commit -m "feat(langsmith): add batch exporter"
```
---
### Task 10: LangSmith callback handler
**Files:**
- Create: `wesichain-langsmith/src/handler.rs`
- Create: `wesichain-langsmith/tests/handler.rs`
**Step 1: Write failing tests**
Create `wesichain-langsmith/tests/handler.rs`:
```rust
use std::sync::Arc;
use std::time::Duration;
use secrecy::SecretString;
use serde_json::json;
use uuid::Uuid;
use wesichain_core::callbacks::{CallbackHandler, RunContext, RunType};
use wesichain_langsmith::{LangSmithCallbackHandler, LangSmithConfig, Sampler};
struct NeverSampler;
impl Sampler for NeverSampler {
    fn should_sample(&self, _run_id: Uuid) -> bool {
        false
    }
}
#[tokio::test]
async fn sampling_short_circuits_before_enqueue() {
    let config = LangSmithConfig {
        api_key: SecretString::new("key".to_string()),
        api_url: "http://localhost".to_string(),
        project_name: "test".to_string(),
        flush_interval: Duration::from_secs(3600),
        max_batch_size: 10,
        queue_capacity: 10,
        sampling_rate: 1.0,
        redact_regex: None,
    };
    let handler = LangSmithCallbackHandler::with_sampler(config, Arc::new(NeverSampler));
    let ctx = RunContext::root(RunType::Chain, "node".to_string(), vec![], Default::default());
    handler.on_start(&ctx, &json!({"x": 1})).await;
    let stats = handler.flush(Duration::from_millis(50)).await.unwrap();
    assert_eq!(stats.runs_flushed, 0);
}
```
**Step 2: Run tests to verify failure**
Run: `cargo test -p wesichain-langsmith handler -v`
Expected: FAIL
**Step 3: Implement LangSmithCallbackHandler**
Create `wesichain-langsmith/src/handler.rs`:
- Implement `wesichain_core::callbacks::CallbackHandler`.
- Map `RunContext` to `RunEvent::Start` and `RunEvent::Update`.
- Use `sanitize_value` + `truncate_value` + `ensure_object` before enqueue.
- Use `ProbabilitySampler` by default; allow `with_sampler` override.
- Track per-trace sampling to drop full trace trees.
- Expose `flush` and `dropped_events` on the handler.
**Step 4: Run tests to verify pass**
Run: `cargo test -p wesichain-langsmith handler -v`
Expected: PASS
**Step 5: Commit**
```bash
git add wesichain-langsmith/src/handler.rs wesichain-langsmith/tests/handler.rs
git commit -m "feat(langsmith): add callback handler"
```
---
### Task 11: Integration test with graph or agent
**Files:**
- Create: `wesichain-langsmith/tests/integration.rs`
**Step 1: Write failing test**
Create `wesichain-langsmith/tests/integration.rs` that:
- Builds a small graph or agent with callbacks enabled.
- Uses wiremock to accept POST/PATCH on `/runs`.
- Asserts both POST and PATCH requests were received.
**Step 2: Run tests to verify failure**
Run: `cargo test -p wesichain-langsmith integration -v`
Expected: FAIL
**Step 3: Implement missing pieces**
Adjust handler/exporter or graph/agent instrumentation if needed to ensure integration passes.
**Step 4: Run tests to verify pass**
Run: `cargo test -p wesichain-langsmith integration -v`
Expected: PASS
**Step 5: Commit**
```bash
git add wesichain-langsmith/tests/integration.rs
git commit -m "test(langsmith): add integration trace"
```
---
### Task 12: Documentation and usage example
**Files:**
- Modify: `wesichain-langsmith/src/lib.rs`
**Step 1: Add crate-level docs**
Add an example showing how to construct `CallbackManager`, attach `LangSmithCallbackHandler`, and run a graph with `ExecutionOptions::run_config`.
**Step 2: Verify docs build**
Run: `cargo test -p wesichain-langsmith --doc`
Expected: PASS
**Step 3: Commit**
```bash
git add wesichain-langsmith/src/lib.rs
git commit -m "docs(langsmith): add usage example"
```
---
## Validation Checklist
- [ ] Root and child runs share trace_id and correct parent_run_id.
- [ ] Inputs/outputs are JSON objects (wrapped when primitive).
- [ ] Errors are terminal and visible in LangSmith UI.
- [ ] Sampling drops entire traces (no orphans).
- [ ] Backpressure increments dropped counters without blocking.
- [ ] Wiremock tests show POST and PATCH calls.
## Suggested Verification Commands
- `cargo test -p wesichain-core callbacks traced_runnable -v`
- `cargo test -p wesichain-graph callbacks -v`
- `cargo test -p wesichain-agent callbacks -v`
- `cargo test -p wesichain-langsmith -v`
- `cargo test`
