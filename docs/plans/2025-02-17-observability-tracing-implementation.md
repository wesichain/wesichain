# Observability/Tracing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add structured LLM observability types (TokenUsage, LlmInput, LlmResult) and extend CallbackHandler trait with on_llm_start/on_llm_end callbacks, fix streaming callbacks, and update LangSmith integration.

**Architecture:** Add new types to wesichain-core callbacks module, extend CallbackHandler with default impls for backward compatibility, fix TracedRunnable streaming bug, then update LangSmithCallbackHandler to consume structured data.

**Tech Stack:** Rust, async-trait, serde_json, tokio

---

## Prerequisites

- Working directory: `/Users/bene/Documents/bene/python/rechain/wesichain`
- All commands run from this directory
- Tests run with: `cargo test <test_name> --package wesichain-core` (or appropriate package)

---

### Task 1: Create LLM Observability Types

**Files:**
- Create: `wesichain-core/src/callbacks/llm.rs`
- Modify: `wesichain-core/src/callbacks/mod.rs` (add module export)
- Test: `wesichain-core/tests/callbacks_llm.rs`

**Step 1: Write the failing test**

Create `wesichain-core/tests/callbacks_llm.rs`:

```rust
use wesichain_core::{LlmInput, LlmResult, TokenUsage};

#[test]
fn token_usage_creation() {
    let usage = TokenUsage {
        prompt_tokens: 10,
        completion_tokens: 20,
        total_tokens: 30,
    };
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 20);
    assert_eq!(usage.total_tokens, 30);
}

#[test]
fn llm_input_creation() {
    let input = LlmInput {
        model: "gpt-4".to_string(),
        prompt: "Hello, world!".to_string(),
        temperature: Some(0.7),
        max_tokens: Some(100),
        stop_sequences: vec!["\n".to_string()],
    };
    assert_eq!(input.model, "gpt-4");
    assert_eq!(input.prompt, "Hello, world!");
    assert_eq!(input.temperature, Some(0.7));
    assert_eq!(input.max_tokens, Some(100));
    assert_eq!(input.stop_sequences, vec!["\n"]);
}

#[test]
fn llm_result_with_token_usage() {
    let result = LlmResult {
        token_usage: Some(TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        }),
        model: "gpt-4".to_string(),
        finish_reason: Some("stop".to_string()),
        generations: vec!["Hi there!".to_string()],
    };
    assert!(result.token_usage.is_some());
    assert_eq!(result.token_usage.as_ref().unwrap().total_tokens, 30);
}

#[test]
fn llm_result_without_token_usage() {
    let result = LlmResult {
        token_usage: None,
        model: "local-model".to_string(),
        finish_reason: None,
        generations: vec![],
    };
    assert!(result.token_usage.is_none());
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test --package wesichain-core token_usage_creation -- --nocapture
```

Expected: FAIL with "TokenUsage not found" or similar

**Step 3: Create the types**

Create `wesichain-core/src/callbacks/llm.rs`:

```rust
//! Structured types for LLM observability.
//!
//! These types capture LLM-specific inputs and outputs for cost tracking,
//! prompt debugging, and performance analysis.

/// Token consumption for cost tracking and optimization.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// LLM call parameters captured at start time.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LlmInput {
    pub model: String,
    /// Rendered prompt (after template expansion), not the template itself
    pub prompt: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
}

/// LLM call results captured at end time.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LlmResult {
    pub token_usage: Option<TokenUsage>,
    pub model: String,
    pub finish_reason: Option<String>,
    /// Rendered output strings (one per generation)
    pub generations: Vec<String>,
}
```

**Step 4: Add module to callbacks/mod.rs**

Modify `wesichain-core/src/callbacks/mod.rs`:

Add at the top:
```rust
mod llm;
```

Add to the pub use statements (around line 27-28):
```rust
pub use llm::{LlmInput, LlmResult, TokenUsage};
```

**Step 5: Run tests to verify**

```bash
cargo test --package wesichain-core --test callbacks_llm -- --nocapture
```

Expected: All 4 tests PASS

**Step 6: Commit**

```bash
git add wesichain-core/src/callbacks/llm.rs wesichain-core/src/callbacks/mod.rs wesichain-core/tests/callbacks_llm.rs
git commit -m "feat(core): add TokenUsage, LlmInput, LlmResult types

Add structured types for LLM observability to enable cost tracking
and prompt debugging in callback handlers.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 2: Extend CallbackHandler Trait

**Files:**
- Modify: `wesichain-core/src/callbacks/mod.rs` (extend trait)
- Test: `wesichain-core/tests/callbacks_llm_handler.rs`

**Step 1: Write the failing test**

Create `wesichain-core/tests/callbacks_llm_handler.rs`:

```rust
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use wesichain_core::{
    CallbackHandler, CallbackManager, LlmInput, LlmResult, RunContext, RunType, TokenUsage, Value,
};

#[derive(Clone)]
struct RecordingHandler {
    llm_starts: Arc<Mutex<Vec<(String, LlmInput)>>>,
    llm_ends: Arc<Mutex<Vec<(String, LlmResult)>>>,
    generic_starts: Arc<Mutex<Vec<String>>>,
    generic_ends: Arc<Mutex<Vec<String>>>,
}

impl RecordingHandler {
    fn new() -> Self {
        Self {
            llm_starts: Arc::new(Mutex::new(Vec::new())),
            llm_ends: Arc::new(Mutex::new(Vec::new())),
            generic_starts: Arc::new(Mutex::new(Vec::new())),
            generic_ends: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl CallbackHandler for RecordingHandler {
    async fn on_start(&self, ctx: &RunContext, _inputs: &Value) {
        self.generic_starts.lock().unwrap().push(ctx.name.clone());
    }

    async fn on_end(&self, ctx: &RunContext, _outputs: &Value, _duration_ms: u128) {
        self.generic_ends.lock().unwrap().push(ctx.name.clone());
    }

    async fn on_error(&self, _ctx: &RunContext, _error: &Value, _duration_ms: u128) {}

    async fn on_llm_start(&self, ctx: &RunContext, input: &LlmInput) {
        self.llm_starts
            .lock()
            .unwrap()
            .push((ctx.name.clone(), input.clone()));
    }

    async fn on_llm_end(&self, ctx: &RunContext, result: &LlmResult, _duration_ms: u128) {
        self.llm_ends
            .lock()
            .unwrap()
            .push((ctx.name.clone(), result.clone()));
    }
}

#[tokio::test]
async fn callback_handler_llm_methods_are_called() {
    let handler = Arc::new(RecordingHandler::new());
    let manager = CallbackManager::new(vec![handler.clone()]);

    let ctx = RunContext::root(RunType::Llm, "test-llm".to_string(), vec![], BTreeMap::new());
    let input = LlmInput {
        model: "gpt-4".to_string(),
        prompt: "Hello".to_string(),
        temperature: Some(0.5),
        max_tokens: Some(100),
        stop_sequences: vec![],
    };

    manager.on_llm_start(&ctx, &input).await;

    let starts = handler.llm_starts.lock().unwrap();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].0, "test-llm");
    assert_eq!(starts[0].1.model, "gpt-4");
}

#[tokio::test]
async fn default_impl_fallback_to_on_start() {
    struct FallbackHandler {
        starts: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl CallbackHandler for FallbackHandler {
        async fn on_start(&self, ctx: &RunContext, _inputs: &Value) {
            self.starts.lock().unwrap().push(ctx.name.clone());
        }
        async fn on_end(&self, _ctx: &RunContext, _outputs: &Value, _duration_ms: u128) {}
        async fn on_error(&self, _ctx: &RunContext, _error: &Value, _duration_ms: u128) {}
    }

    let handler = Arc::new(FallbackHandler {
        starts: Arc::new(Mutex::new(Vec::new())),
    });
    let manager = CallbackManager::new(vec![handler.clone()]);

    let ctx = RunContext::root(RunType::Llm, "fallback-test".to_string(), vec![], BTreeMap::new());
    let input = LlmInput {
        model: "gpt-3".to_string(),
        prompt: "Test".to_string(),
        temperature: None,
        max_tokens: None,
        stop_sequences: vec![],
    };

    // Calls on_llm_start which has default impl calling on_start
    manager.on_llm_start(&ctx, &input).await;

    let starts = handler.starts.lock().unwrap();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0], "fallback-test");
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test --package wesichain-core callback_handler_llm_methods_are_called -- --nocapture
```

Expected: FAIL with "no method named `on_llm_start`" or similar

**Step 3: Extend CallbackHandler trait**

Modify `wesichain-core/src/callbacks/mod.rs`:

Add the import at the top:
```rust
use llm::{LlmInput, LlmResult};
```

Extend the CallbackHandler trait (after the existing methods):

```rust
#[async_trait]
pub trait CallbackHandler: Send + Sync {
    async fn on_start(&self, ctx: &RunContext, inputs: &Value);
    async fn on_end(&self, ctx: &RunContext, outputs: &Value, duration_ms: u128);
    async fn on_error(&self, ctx: &RunContext, error: &Value, duration_ms: u128);
    async fn on_stream_chunk(&self, _ctx: &RunContext, _chunk: &Value) {}

    /// Called when an LLM call starts. Override for structured LLM observability.
    /// Default implementation calls `on_start` with serialized input.
    async fn on_llm_start(&self, ctx: &RunContext, input: &LlmInput) {
        self.on_start(ctx, &serde_json::to_value(input).unwrap_or_default()).await
    }

    /// Called when an LLM call ends. Override for structured LLM observability.
    /// Default implementation calls `on_end` with serialized result.
    async fn on_llm_end(&self, ctx: &RunContext, result: &LlmResult, duration_ms: u128) {
        self.on_end(ctx, &serde_json::to_value(result).unwrap_or_default(), duration_ms).await
    }
}
```

**Step 4: Add forwarding methods to CallbackManager**

Add to `CallbackManager` impl in `wesichain-core/src/callbacks/mod.rs`:

```rust
impl CallbackManager {
    // ... existing methods ...

    pub async fn on_llm_start(&self, ctx: &RunContext, input: &LlmInput) {
        for handler in &self.handlers {
            handler.on_llm_start(ctx, input).await;
        }
    }

    pub async fn on_llm_end(&self, ctx: &RunContext, result: &LlmResult, duration_ms: u128) {
        for handler in &self.handlers {
            handler.on_llm_end(ctx, result, duration_ms).await;
        }
    }
}
```

**Step 5: Run tests to verify**

```bash
cargo test --package wesichain-core --test callbacks_llm_handler -- --nocapture
```

Expected: Both tests PASS

**Step 6: Commit**

```bash
git add wesichain-core/src/callbacks/mod.rs wesichain-core/tests/callbacks_llm_handler.rs
git commit -m "feat(core): add on_llm_start/on_llm_end to CallbackHandler

Add structured LLM callbacks with default implementations for backward
compatibility. Includes CallbackManager forwarding methods.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 3: Fix Streaming Callbacks in TracedRunnable

**Files:**
- Modify: `wesichain-core/src/callbacks/wrappers.rs`
- Test: `wesichain-core/tests/traced_runnable_streaming.rs`

**Step 1: Write the failing test**

Create `wesichain-core/tests/traced_runnable_streaming.rs`:

```rust
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use futures::stream::{self, BoxStream, StreamExt};
use wesichain_core::{
    CallbackHandler, CallbackManager, RunContext, RunType, StreamEvent, Value, WesichainError,
};

#[derive(Clone)]
struct RecordingHandler {
    events: Arc<Mutex<Vec<String>>>,
}

impl RecordingHandler {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }
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

    async fn on_stream_chunk(&self, _ctx: &RunContext, _chunk: &Value) {
        self.events.lock().unwrap().push("chunk".to_string());
    }
}

struct MockStreamingRunnable;

#[async_trait::async_trait]
impl wesichain_core::Runnable<String, String> for MockStreamingRunnable {
    async fn invoke(&self, _input: String) -> Result<String, WesichainError> {
        Ok("result".to_string())
    }

    fn stream(
        &self,
        _input: String,
    ) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::iter(vec![
            Ok(StreamEvent::Chunk(Value::String("Hello".to_string()))),
            Ok(StreamEvent::Chunk(Value::String(" World".to_string()))),
        ])
        .boxed()
    }
}

#[tokio::test]
async fn traced_runnable_stream_fires_callbacks() {
    use wesichain_core::{Runnable, RunConfig, ToTraceInput, ToTraceOutput};
    use wesichain_core::callbacks::wrappers::TracedRunnable;

    let handler = Arc::new(RecordingHandler::new());
    let manager = CallbackManager::new(vec![handler.clone()]);

    let parent = RunContext::root(RunType::Chain, "parent".to_string(), vec![], BTreeMap::new());

    let inner = MockStreamingRunnable;
    let traced = TracedRunnable::new(
        inner,
        manager,
        parent,
        RunType::Chain,
        "test".to_string(),
    );

    let mut stream = traced.stream("input".to_string());
    while let Some(_event) = stream.next().await {}

    let events = handler.events.lock().unwrap();
    assert!(
        events.contains(&"start".to_string()),
        "should have fired on_start"
    );
    assert!(
        events.contains(&"chunk".to_string()),
        "should have fired on_stream_chunk"
    );
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test --package wesichain-core traced_runnable_stream_fires_callbacks -- --nocapture
```

Expected: FAIL - "start" not in events (current implementation passes through without callbacks)

**Step 3: Implement streaming callbacks in TracedRunnable**

Read `wesichain-core/src/callbacks/wrappers.rs` first to understand current implementation.

Modify `wesichain-core/src/callbacks/wrappers.rs`:

Replace the `stream` method implementation:

```rust
use futures::stream::BoxStream;
use futures::StreamExt;

use crate::callbacks::{
    ensure_object, CallbackManager, RunContext, RunType, ToTraceInput, ToTraceOutput,
};
use crate::{Runnable, StreamEvent, WesichainError};

// ... existing TracedRunnable struct ...

impl<R> TracedRunnable<R> {
    // ... existing new() method ...
}

#[async_trait::async_trait]
impl<Input, Output, R> Runnable<Input, Output> for TracedRunnable<R>
where
    Input: Send + Sync + ToTraceInput + 'static,
    Output: Send + Sync + ToTraceOutput + 'static,
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
        let duration_ms = ctx.start_instant.elapsed().as_millis();

        match &result {
            Ok(output) => {
                let outputs = ensure_object(output.to_trace_output());
                self.manager.on_end(&ctx, &outputs, duration_ms).await;
            }
            Err(err) => {
                let error = ensure_object(err.to_string().to_trace_output());
                self.manager.on_error(&ctx, &error, duration_ms).await;
            }
        }

        result
    }

    fn stream(&self, input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        if self.manager.is_noop() {
            return self.inner.stream(input);
        }

        let manager = self.manager.clone();
        let parent = self.parent.clone();
        let run_type = self.run_type.clone();
        let name = self.name.clone();
        let inner_stream = self.inner.stream(input);

        Box::pin(async_stream::stream! {
            let ctx = parent.child(run_type, name);
            let inputs = ensure_object(input.to_trace_input());
            manager.on_start(&ctx, &inputs).await;

            let mut last_result: Option<Result<StreamEvent, WesichainError>> = None;

            for await event in inner_stream {
                match &event {
                    Ok(StreamEvent::Chunk(chunk)) => {
                        manager.on_stream_chunk(&ctx, chunk).await;
                    }
                    Ok(StreamEvent::Done(final_output)) => {
                        let outputs = ensure_object(final_output.to_trace_output());
                        let duration_ms = ctx.start_instant.elapsed().as_millis();
                        manager.on_end(&ctx, &outputs, duration_ms).await;
                    }
                    Err(err) => {
                        let error = ensure_object(err.to_string().to_trace_output());
                        let duration_ms = ctx.start_instant.elapsed().as_millis();
                        manager.on_error(&ctx, &error, duration_ms).await;
                    }
                }
                last_result = Some(event.clone());
                yield event;
            }

            // If stream ended without Done event, call on_end
            if let Some(Ok(StreamEvent::Chunk(_))) = last_result {
                let outputs = ensure_object(Value::Object(serde_json::Map::new()));
                let duration_ms = ctx.start_instant.elapsed().as_millis();
                manager.on_end(&ctx, &outputs, duration_ms).await;
            }
        })
    }
}
```

**Note:** This requires `async-stream` crate. Check if it's already in Cargo.toml:

```bash
grep "async-stream" wesichain-core/Cargo.toml
```

If not present, add to `wesichain-core/Cargo.toml`:

```toml
[dependencies]
# ... existing deps ...
async-stream = "0.3"
futures = "0.3"
```

**Step 4: Run tests to verify**

```bash
cargo test --package wesichain-core --test traced_runnable_streaming -- --nocapture
```

Expected: Test PASS

**Step 5: Commit**

```bash
git add wesichain-core/src/callbacks/wrappers.rs wesichain-core/tests/traced_runnable_streaming.rs wesichain-core/Cargo.toml
git commit -m "fix(core): implement streaming callbacks in TracedRunnable

TracedRunnable::stream() now properly fires on_start, on_stream_chunk,
and on_end/on_error callbacks. Previously it passed through to inner
without any callback invocation.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 4: Update LangSmithCallbackHandler

**Files:**
- Modify: `wesichain-langsmith/src/handler.rs`
- Test: `wesichain-langsmith/tests/handler_llm.rs`

**Step 1: Write the failing test**

Create `wesichain-langsmith/tests/handler_llm.rs`:

```rust
use std::collections::BTreeMap;
use std::time::Duration;

use secrecy::SecretString;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wesichain_core::{CallbackHandler, LlmInput, LlmResult, RunContext, RunType, TokenUsage};
use wesichain_langsmith::{LangSmithCallbackHandler, LangSmithConfig};

#[tokio::test]
async fn handler_emits_llm_start_event() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/runs"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&mock_server)
        .await;

    let config = LangSmithConfig {
        api_key: SecretString::new("test-key".to_string()),
        api_url: mock_server.uri(),
        project_name: "test-project".to_string(),
        ..Default::default()
    };

    let handler = LangSmithCallbackHandler::new(config);

    let ctx = RunContext::root(RunType::Llm, "test-llm".to_string(), vec![], BTreeMap::new());
    let input = LlmInput {
        model: "gpt-4".to_string(),
        prompt: "Hello".to_string(),
        temperature: Some(0.7),
        max_tokens: Some(100),
        stop_sequences: vec![],
    };

    handler.on_llm_start(&ctx, &input).await;

    // Flush to ensure event is sent
    let _ = handler.flush(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn handler_emits_llm_end_with_token_usage() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/runs"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&mock_server)
        .await;

    Mock::given(method("PATCH"))
        .and(path("/runs/.+"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&mock_server)
        .await;

    let config = LangSmithConfig {
        api_key: SecretString::new("test-key".to_string()),
        api_url: mock_server.uri(),
        project_name: "test-project".to_string(),
        ..Default::default()
    };

    let handler = LangSmithCallbackHandler::new(config);

    // First call on_llm_start
    let ctx = RunContext::root(RunType::Llm, "test-llm".to_string(), vec![], BTreeMap::new());
    let input = LlmInput {
        model: "gpt-4".to_string(),
        prompt: "Hello".to_string(),
        temperature: None,
        max_tokens: None,
        stop_sequences: vec![],
    };
    handler.on_llm_start(&ctx, &input).await;

    // Then call on_llm_end with token usage
    let result = LlmResult {
        token_usage: Some(TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        }),
        model: "gpt-4".to_string(),
        finish_reason: Some("stop".to_string()),
        generations: vec!["Hi".to_string()],
    };
    handler.on_llm_end(&ctx, &result, 100).await;

    let _ = handler.flush(Duration::from_secs(1)).await;
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test --package wesichain-langsmith handler_emits_llm_start_event -- --nocapture
```

Expected: FAIL - handler doesn't implement on_llm_start yet (uses default)

**Step 3: Implement on_llm_start/on_llm_end in LangSmithCallbackHandler**

Modify `wesichain-langsmith/src/handler.rs`:

Add to imports:
```rust
use wesichain_core::{LlmInput, LlmResult};
```

Add new methods to `LangSmithCallbackHandler` impl:

```rust
impl LangSmithCallbackHandler {
    // ... existing methods ...

    fn prepare_llm_inputs(&self, input: &LlmInput) -> Value {
        let mut metadata = serde_json::Map::new();
        if let Some(temp) = input.temperature {
            metadata.insert("temperature".to_string(), json!(temp));
        }
        if let Some(max_tokens) = input.max_tokens {
            metadata.insert("max_tokens".to_string(), json!(max_tokens));
        }
        if !input.stop_sequences.is_empty() {
            metadata.insert("stop".to_string(), json!(input.stop_sequences));
        }

        json!({
            "model": input.model,
            "prompt": self.sanitize_object(Value::String(input.prompt.clone())),
            "invocation_params": metadata,
        })
    }

    fn prepare_llm_outputs(&self, result: &LlmResult) -> (Value, Option<Value>) {
        let outputs = json!({
            "generations": result.generations,
            "model": result.model,
        });

        let usage = result.token_usage.as_ref().map(|u| {
            json!({
                "prompt_tokens": u.prompt_tokens,
                "completion_tokens": u.completion_tokens,
                "total_tokens": u.total_tokens,
            })
        });

        (self.sanitize_object(outputs), usage)
    }
}
```

Add to the `CallbackHandler` impl:

```rust
#[async_trait::async_trait]
impl CallbackHandler for LangSmithCallbackHandler {
    // ... existing on_start, on_end, on_error ...

    async fn on_llm_start(&self, ctx: &RunContext, input: &LlmInput) {
        if !self.should_sample(ctx.trace_id) {
            return;
        }

        let inputs = self.prepare_llm_inputs(input);
        let metadata = serde_json::to_value(&ctx.metadata).unwrap_or(Value::Null);
        let metadata = self.sanitize_object(metadata);

        let event = RunEvent::Start {
            run_id: ctx.run_id,
            parent_run_id: ctx.parent_run_id,
            trace_id: ctx.trace_id,
            name: ctx.name.clone(),
            run_type: RunType::Llm,
            start_time: DateTime::<Utc>::from(ctx.start_time),
            inputs,
            tags: ctx.tags.clone(),
            metadata,
            session_name: self.session_name.clone(),
        };
        self.exporter.enqueue(event).await;
    }

    async fn on_llm_end(&self, ctx: &RunContext, result: &LlmResult, duration_ms: u128) {
        if !self.should_sample(ctx.trace_id) {
            self.maybe_clear_trace(ctx);
            return;
        }

        let (outputs, token_usage) = self.prepare_llm_outputs(result);

        // Build extra field with duration and token usage
        let mut extra = serde_json::Map::new();
        extra.insert("duration_ms".to_string(), json!(duration_ms as u64));
        if let Some(usage) = token_usage {
            extra.insert("token_usage".to_string(), usage);
        }

        let event = RunEvent::Update {
            run_id: ctx.run_id,
            end_time: Some(Utc::now()),
            outputs: Some(outputs),
            error: None,
            duration_ms: Some(duration_ms),
        };
        self.exporter.enqueue(event).await;
        self.maybe_clear_trace(ctx);
    }
}
```

**Step 4: Run tests to verify**

```bash
cargo test --package wesichain-langsmith --test handler_llm -- --nocapture
```

Expected: Tests PASS

**Step 5: Verify existing tests still pass**

```bash
cargo test --package wesichain-langsmith
```

Expected: All tests PASS

**Step 6: Commit**

```bash
git add wesichain-langsmith/src/handler.rs wesichain-langsmith/tests/handler_llm.rs
git commit -m "feat(langsmith): implement on_llm_start/on_llm_end

Add structured LLM callback handlers that emit proper LangSmith events
with model info, temperature, max_tokens, and token usage tracking.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Final Verification

Run the full test suite for affected packages:

```bash
cargo test --package wesichain-core
cargo test --package wesichain-langsmith
```

All tests should pass.

---

## Post-Implementation

After this plan is complete, the following will be ready:

1. ✅ `TokenUsage`, `LlmInput`, `LlmResult` types in `wesichain-core`
2. ✅ `CallbackHandler` extended with `on_llm_start/on_llm_end` with default impls
3. ✅ Streaming callbacks fixed in `TracedRunnable`
4. ✅ `LangSmithCallbackHandler` uses structured LLM data

Next steps (future work):
- Update LLM providers to emit `on_llm_start/on_llm_end` instead of generic `on_start/on_end`
- Design `wesichain-opentelemetry` crate
- Graceful shutdown flush for LangSmith
