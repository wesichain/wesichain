---
name: wesichain-core
description: |
  Core abstractions for Wesichain: Runnable trait, Chain composition, streaming,
  typed errors, and retry logic. Use this skill when building composable LLM
  components with LCEL-style chaining in Rust.
triggers:
  - "runnable"
  - "chain"
  - "then()"
  - "stream"
  - "wesichain-core"
  - "composition"
  - "retry"
---

## When to Use

Use wesichain-core when you need to:
- Build composable LLM pipelines with type-safe chaining
- Implement custom components that plug into the Runnable ecosystem
- Add retry logic, streaming, or error handling to your chains
- Create reusable, testable LLM components in Rust

## Quick Start

```rust
use wesichain_core::{Runnable, RunnableExt, WesichainError};

// Define a simple Runnable
struct AddPrefix;

#[async_trait::async_trait]
impl Runnable<String, String> for AddPrefix {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(format!("pre-{input}"))
    }

    fn stream(&self, input: String) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        // Implementation for streaming
        futures::stream::iter(vec![
            Ok(StreamEvent::FinalAnswer(format!("pre-{input}")))
        ]).boxed()
    }
}

// Chain multiple runnables together
let chain = AddPrefix.then(Uppercase).then(AddSuffix);
let result = chain.invoke("hello".to_string()).await?;
// Result: "PRE-HELLO-suf"
```

## Key Patterns

### Pattern 1: Runnable Implementation

```rust
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use futures::stream::BoxStream;

#[async_trait::async_trait]
impl Runnable<Input, Output> for MyComponent {
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        // Transform input to output
        Ok(transform(input))
    }

    fn stream(&self, input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        // Emit streaming events
        futures::stream::iter(events).boxed()
    }
}
```

### Pattern 2: Chain Composition with .then()

```rust
use wesichain_core::RunnableExt;

// Compose runnables into a pipeline
let pipeline = PromptTemplate::new("Hello {name}")
    .then(DummyLlm)
    .then(SimpleParser)
    .with_retries(2);

let result = pipeline.invoke(json!({"name": "World"})).await?;
```

### Pattern 3: Streaming Events

```rust
use wesichain_core::Runnable;
use futures::StreamExt;

let mut events = chain.stream("input".to_string());
while let Some(event) = events.next().await {
    match event? {
        StreamEvent::ContentChunk(chunk) => print!("{}", chunk),
        StreamEvent::ToolCallStart { name, .. } => println!("Calling: {}", name),
        StreamEvent::FinalAnswer(answer) => println!("\nFinal: {}", answer),
        _ => {}
    }
}
```

### Pattern 4: Error Handling with WesichainError

```rust
use wesichain_core::WesichainError;

// Match on specific error types
match result {
    Err(WesichainError::MaxRetriesExceeded { max }) => {
        eprintln!("Failed after {} attempts", max);
    }
    Err(WesichainError::LlmProvider(msg)) => {
        eprintln!("LLM error: {}", msg);
    }
    Err(WesichainError::ParseFailed { output, reason }) => {
        eprintln!("Parse failed on '{}': {}", output, reason);
    }
    Ok(value) => println!("Success: {}", value),
}
```

## Golden Rules

1. **Always implement both `invoke` and `stream`** - Even if streaming just returns a single FinalAnswer event
2. **Use `RunnableExt` for composition** - The `.then()` method is the primary way to chain components
3. **Prefer typed errors** - Return `WesichainError` variants instead of generic strings for better error handling
4. **Make components Send + Sync** - All Runnables must be thread-safe for async execution
5. **Use `with_retries()` for flaky operations** - Wrap external API calls (LLMs, tools) with retry logic

## Common Mistakes

- **Implementing `stream` that ignores the input** - Always use the input parameter in your stream implementation
- **Forgetting Send + Sync bounds** - Runnables must be thread-safe; use Arc for shared state
- **Mixing error types** - Convert external errors to WesichainError using `?` or `.map_err()`
- **Blocking in async methods** - Use `tokio::task::spawn_blocking` for CPU-intensive work
- **Not handling cancellation** - Check for cancellation tokens in long-running operations

## Resources

- Full guide: `/Users/bene/Documents/bene/python/rechain/wesichain/.worktrees/ai-skills-docs/docs/plans/2026-02-02-wesichain-phase-1-core-implementation-plan.md`
- Crate: `wesichain-core`
- Key traits: `Runnable`, `RunnableExt`, `RetryExt`
- Key types: `WesichainError`, `StreamEvent`, `Value`
