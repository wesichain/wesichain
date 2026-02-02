# Wesichain Phase 1 Core Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the Phase 1 core abstraction layer in `wesichain-core` with typed `Runnable`, `.then()` composition, structured streaming, typed errors, retries, and `Value` helpers.

**Architecture:** Implement a generic `Runnable<Input, Output>` using `async-trait`, a binary `Chain<A, B>` composed via `.then()`, a structured `StreamEvent` enum, a typed `WesichainError` enum, and a minimal retry wrapper exposed by `RunnableExt::with_retries`. Provide a `Value` alias to `serde_json::Value` plus conversion helpers for quick prototyping.

**Tech Stack:** Rust 1.75+, async-trait, futures (BoxStream), serde + serde_json, thiserror, tokio (test runtime).

**Skills:** Use @superpowers:executing-plans to implement; use @superpowers:verification-before-completion before claiming tests pass.

---

### Task 1: Add WesichainError (typed error model)

**Files:**
- Create: `wesichain-core/src/error.rs`
- Modify: `wesichain-core/src/lib.rs`
- Modify: `wesichain-core/Cargo.toml`
- Test: `wesichain-core/tests/error.rs`

**Step 1: Write the failing test**

```rust
use wesichain_core::WesichainError;

#[test]
fn error_display_for_max_retries() {
    let err = WesichainError::MaxRetriesExceeded { max: 2 };
    assert_eq!(format!("{err}"), "Max retries (2) exceeded");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-core --test error -v`
Expected: FAIL with unresolved import or missing `WesichainError`.

**Step 3: Write minimal implementation**

```rust
// wesichain-core/src/error.rs
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WesichainError {
    #[error("LLM provider failed: {0}")]
    LlmProvider(String),

    #[error("Tool call failed for '{tool_name}': {reason}")]
    ToolCallFailed { tool_name: String, reason: String },

    #[error("Parsing failed on output '{output}': {reason}")]
    ParseFailed { output: String, reason: String },

    #[error("Operation timed out after {0:?}")]
    Timeout(Duration),

    #[error("Max retries ({max}) exceeded")]
    MaxRetriesExceeded { max: usize },

    #[error("Checkpoint failed: {0}")]
    CheckpointFailed(String),

    #[error("Operation was cancelled")]
    Cancelled,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Serialization/deserialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    Custom(String),
}
```

```rust
// wesichain-core/src/lib.rs
mod error;

pub use error::WesichainError;
```

```toml
# wesichain-core/Cargo.toml
[dependencies]
thiserror = "1"
serde_json = "1"
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-core --test error -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-core/src/error.rs wesichain-core/src/lib.rs wesichain-core/Cargo.toml wesichain-core/tests/error.rs
git commit -m "feat(core): add WesichainError"
```

### Task 2: Add Value alias and conversion helpers

**Files:**
- Create: `wesichain-core/src/value.rs`
- Modify: `wesichain-core/src/lib.rs`
- Modify: `wesichain-core/Cargo.toml`
- Test: `wesichain-core/tests/value.rs`

**Step 1: Write the failing test**

```rust
use serde::{Deserialize, Serialize};
use wesichain_core::{TryFromValue, Value, IntoValue};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Demo {
    name: String,
}

#[test]
fn value_roundtrip_for_struct() {
    let input = Demo { name: "alpha".to_string() };
    let value: Value = input.into_value();
    let output = Demo::try_from_value(value).expect("convert back");
    assert_eq!(output, Demo { name: "alpha".to_string() });
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-core --test value -v`
Expected: FAIL with missing `Value` and traits.

**Step 3: Write minimal implementation**

```rust
// wesichain-core/src/value.rs
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::WesichainError;

pub type Value = serde_json::Value;

pub trait IntoValue {
    fn into_value(self) -> Value;
}

impl<T> IntoValue for T
where
    T: Serialize,
{
    fn into_value(self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

pub trait TryFromValue: Sized {
    fn try_from_value(value: Value) -> Result<Self, WesichainError>;
}

impl<T> TryFromValue for T
where
    T: DeserializeOwned,
{
    fn try_from_value(value: Value) -> Result<Self, WesichainError> {
        Ok(serde_json::from_value(value)?)
    }
}
```

```rust
// wesichain-core/src/lib.rs
mod value;

pub use value::{IntoValue, TryFromValue, Value};
```

```toml
# wesichain-core/Cargo.toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-core --test value -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-core/src/value.rs wesichain-core/src/lib.rs wesichain-core/Cargo.toml wesichain-core/tests/value.rs
git commit -m "feat(core): add Value helpers"
```

### Task 3: Add StreamEvent and Runnable trait

**Files:**
- Create: `wesichain-core/src/runnable.rs`
- Modify: `wesichain-core/src/lib.rs`
- Modify: `wesichain-core/Cargo.toml`
- Test: `wesichain-core/tests/runnable_stream.rs`

**Step 1: Write the failing test**

```rust
use futures::StreamExt;
use wesichain_core::{Runnable, StreamEvent, WesichainError};

struct Dummy;

#[async_trait::async_trait]
impl Runnable<String, String> for Dummy {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(format!("{input}!"))
    }

    fn stream(
        &self,
        input: String,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        let events = vec![
            Ok(StreamEvent::ContentChunk(input.clone())),
            Ok(StreamEvent::FinalAnswer(format!("{input}!"))),
        ];
        futures::stream::iter(events).boxed()
    }
}

#[tokio::test]
async fn runnable_stream_emits_events_in_order() {
    let dummy = Dummy;
    let events: Vec<_> = dummy.stream("hi".to_string()).collect().await;
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], Ok(StreamEvent::ContentChunk(_))));
    assert!(matches!(events[1], Ok(StreamEvent::FinalAnswer(_))));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-core --test runnable_stream -v`
Expected: FAIL with missing `Runnable` or `StreamEvent`.

**Step 3: Write minimal implementation**

```rust
// wesichain-core/src/runnable.rs
use futures::stream::BoxStream;

use crate::WesichainError;

#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    ContentChunk(String),
    ToolCallStart { id: String, name: String },
    ToolCallDelta { id: String, delta: crate::Value },
    ToolCallResult { id: String, output: crate::Value },
    FinalAnswer(String),
    Metadata { key: String, value: crate::Value },
}

#[async_trait::async_trait]
pub trait Runnable<Input: Send + 'static, Output: Send + 'static> {
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError>;

    fn stream(&self, input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>>;
}
```

```rust
// wesichain-core/src/lib.rs
mod runnable;

pub use runnable::{Runnable, StreamEvent};
```

```toml
# wesichain-core/Cargo.toml
[dependencies]
async-trait = "0.1"
futures = "0.3"
```

```toml
# wesichain-core/Cargo.toml
[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-core --test runnable_stream -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-core/src/runnable.rs wesichain-core/src/lib.rs wesichain-core/Cargo.toml wesichain-core/tests/runnable_stream.rs
git commit -m "feat(core): add Runnable and StreamEvent"
```

### Task 4: Implement Chain composition and .then()

**Files:**
- Create: `wesichain-core/src/chain.rs`
- Modify: `wesichain-core/src/lib.rs`
- Test: `wesichain-core/tests/chain_invoke.rs`

**Step 1: Write the failing test**

```rust
use wesichain_core::{Runnable, RunnableExt, WesichainError};

struct AddPrefix;
struct Uppercase;
struct AddSuffix;

#[async_trait::async_trait]
impl Runnable<String, String> for AddPrefix {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(format!("pre-{input}"))
    }

    fn stream(
        &self,
        _input: String,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::iter(vec![Ok(wesichain_core::StreamEvent::FinalAnswer("".to_string()))]).boxed()
    }
}

#[async_trait::async_trait]
impl Runnable<String, String> for Uppercase {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(input.to_uppercase())
    }

    fn stream(
        &self,
        _input: String,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::iter(vec![Ok(wesichain_core::StreamEvent::FinalAnswer("".to_string()))]).boxed()
    }
}

#[async_trait::async_trait]
impl Runnable<String, String> for AddSuffix {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(format!("{input}-suf"))
    }

    fn stream(
        &self,
        _input: String,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::iter(vec![Ok(wesichain_core::StreamEvent::FinalAnswer("".to_string()))]).boxed()
    }
}

#[tokio::test]
async fn chain_invokes_in_sequence() {
    let chain = AddPrefix.then(Uppercase).then(AddSuffix);
    let output = chain.invoke("alpha".to_string()).await.unwrap();
    assert_eq!(output, "PRE-ALPHA-suf".to_string());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-core --test chain_invoke -v`
Expected: FAIL with missing `Chain` or `.then()`.

**Step 3: Write minimal implementation**

```rust
// wesichain-core/src/chain.rs
use futures::stream::{self, BoxStream};

use crate::{Runnable, StreamEvent, WesichainError};

pub struct Chain<Head, Tail> {
    head: Head,
    tail: Tail,
}

impl<Head, Tail> Chain<Head, Tail> {
    pub fn new(head: Head, tail: Tail) -> Self {
        Self { head, tail }
    }
}

#[async_trait::async_trait]
impl<Input, Mid, Output, Head, Tail> Runnable<Input, Output> for Chain<Head, Tail>
where
    Input: Send + 'static,
    Mid: Send + 'static,
    Output: Send + 'static,
    Head: Runnable<Input, Mid> + Send + Sync,
    Tail: Runnable<Mid, Output> + Send + Sync,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        let mid = self.head.invoke(input).await?;
        self.tail.invoke(mid).await
    }

    fn stream(&self, input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        let tail = self.tail.stream(self.head.invoke(input).ok().unwrap());
        stream::iter(Vec::<Result<StreamEvent, WesichainError>>::new()).chain(tail).boxed()
    }
}

pub trait RunnableExt<Input, Output>: Runnable<Input, Output> + Sized {
    fn then<NextOutput, Next>(self, next: Next) -> Chain<Self, Next>
    where
        Next: Runnable<Output, NextOutput> + Send + Sync,
        Input: Send + 'static,
        Output: Send + 'static,
        NextOutput: Send + 'static,
    {
        Chain::new(self, next)
    }
}

impl<Input, Output, T> RunnableExt<Input, Output> for T where T: Runnable<Input, Output> + Sized {}
```

```rust
// wesichain-core/src/lib.rs
mod chain;

pub use chain::{Chain, RunnableExt};
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-core --test chain_invoke -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-core/src/chain.rs wesichain-core/src/lib.rs wesichain-core/tests/chain_invoke.rs
git commit -m "feat(core): add Chain composition"
```

### Task 5: Add retry wrapper and .with_retries()

**Files:**
- Create: `wesichain-core/src/retry.rs`
- Modify: `wesichain-core/src/lib.rs`
- Test: `wesichain-core/tests/retry.rs`

**Step 1: Write the failing test**

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

use wesichain_core::{Runnable, RunnableExt, WesichainError};

struct Flaky {
    attempts: AtomicUsize,
    succeed_on: usize,
}

#[async_trait::async_trait]
impl Runnable<String, String> for Flaky {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if attempt < self.succeed_on {
            return Err(WesichainError::LlmProvider("flaky".to_string()));
        }
        Ok(input)
    }

    fn stream(
        &self,
        _input: String,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::iter(vec![Ok(wesichain_core::StreamEvent::FinalAnswer("".to_string()))]).boxed()
    }
}

#[tokio::test]
async fn retries_until_success() {
    let flaky = Flaky { attempts: AtomicUsize::new(0), succeed_on: 3 };
    let output = flaky.with_retries(3).invoke("ok".to_string()).await.unwrap();
    assert_eq!(output, "ok".to_string());
}

#[tokio::test]
async fn retries_fail_when_exceeded() {
    let flaky = Flaky { attempts: AtomicUsize::new(0), succeed_on: 5 };
    let err = flaky.with_retries(3).invoke("no".to_string()).await.unwrap_err();
    assert!(matches!(err, WesichainError::MaxRetriesExceeded { .. }));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-core --test retry -v`
Expected: FAIL with missing `.with_retries()`.

**Step 3: Write minimal implementation**

```rust
// wesichain-core/src/retry.rs
use crate::{Runnable, WesichainError};

pub struct Retrying<R> {
    inner: R,
    max_attempts: usize,
}

impl<R> Retrying<R> {
    pub fn new(inner: R, max_attempts: usize) -> Self {
        Self { inner, max_attempts }
    }
}

#[async_trait::async_trait]
impl<Input, Output, R> Runnable<Input, Output> for Retrying<R>
where
    Input: Send + 'static,
    Output: Send + 'static,
    R: Runnable<Input, Output> + Send + Sync,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        let mut last_error = None;
        for _ in 0..self.max_attempts {
            match self.inner.invoke(input).await {
                Ok(output) => return Ok(output),
                Err(err) => {
                    if !is_retryable(&err) {
                        return Err(err);
                    }
                    last_error = Some(err);
                }
            }
        }

        let _ = last_error;
        Err(WesichainError::MaxRetriesExceeded { max: self.max_attempts })
    }

    fn stream(
        &self,
        input: Input,
    ) -> futures::stream::BoxStream<'_, Result<crate::StreamEvent, WesichainError>> {
        self.inner.stream(input)
    }
}

fn is_retryable(err: &WesichainError) -> bool {
    !matches!(err, WesichainError::ParseFailed { .. } | WesichainError::InvalidConfig(_))
}
```

```rust
// wesichain-core/src/lib.rs
mod retry;

pub use retry::Retrying;

pub trait RetryExt<Input, Output>: Runnable<Input, Output> + Sized {
    fn with_retries(self, max_attempts: usize) -> Retrying<Self> {
        Retrying::new(self, max_attempts)
    }
}

impl<Input, Output, T> RetryExt<Input, Output> for T where T: Runnable<Input, Output> + Sized {}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-core --test retry -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-core/src/retry.rs wesichain-core/src/lib.rs wesichain-core/tests/retry.rs
git commit -m "feat(core): add retry wrapper"
```

---

## Phase 1 Acceptance Criteria
- A chain of 3 runnables compiles and executes with `.then()`.
- `Runnable::stream` returns a `BoxStream` and emits structured `StreamEvent`s.
- Errors propagate as `WesichainError` variants; non-retryable errors fail fast.
- `.with_retries()` works for flaky runnables and returns `MaxRetriesExceeded` when exhausted.
- `Value` conversion helpers round-trip a serde-serializable struct.

## Out of Scope (Phase 1)
- LLM provider integration (Ollama/OpenAI).
- Tool registry, memory, or agent loop.
- Timeout wrapper and advanced retry backoff.
- Parallel/branching combinators.
