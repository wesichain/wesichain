# Wesichain

Rust-native LLM agents & chains with resumable ReAct workflows.

## Quick Start – Simple Chain

```toml
[dependencies]
wesichain-core = { path = "wesichain-core" }
async-trait = "0.1"
futures = "0.3"
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Replace the path dependency with a crates.io or git version once published.

```rust
use async_trait::async_trait;
use futures::{stream, StreamExt};
use serde_json::json;
use wesichain_core::{Runnable, RunnableExt, StreamEvent, Value, WesichainError};

struct Prompt;
struct DummyLlm;
struct SimpleParser;

#[async_trait]
impl Runnable<String, Value> for Prompt {
    async fn invoke(&self, input: String) -> Result<Value, WesichainError> {
        Ok(json!({"prompt": input}))
    }

    fn stream(&self, input: String) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::once(async move { Ok(StreamEvent::FinalAnswer(input)) }).boxed()
    }
}

#[async_trait]
impl Runnable<Value, Value> for DummyLlm {
    async fn invoke(&self, input: Value) -> Result<Value, WesichainError> {
        Ok(input)
    }

    fn stream(&self, input: Value) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::once(async move { Ok(StreamEvent::ContentChunk(input.to_string())) }).boxed()
    }
}

#[async_trait]
impl Runnable<Value, String> for SimpleParser {
    async fn invoke(&self, input: Value) -> Result<String, WesichainError> {
        Ok(input["prompt"].as_str().unwrap_or("").to_string())
    }

    fn stream(&self, input: Value) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        let output = input["prompt"].as_str().unwrap_or("").to_string();
        stream::once(async move { Ok(StreamEvent::FinalAnswer(output)) }).boxed()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chain = Prompt.then(DummyLlm).then(SimpleParser).with_retries(2);

    let result = chain.invoke("Tell me a joke".to_string()).await?;
    println!("Result: {result}");

    let mut events = chain.stream("Tell me a joke".to_string());
    while let Some(event) = events.next().await {
        println!("Event: {:?}", event?);
    }

    Ok(())
}
```

Note: in v0, `Chain::stream` forwards events from the tail runnable; the example emits a final answer from `SimpleParser` to demonstrate streaming.

## Status
- v0 design locked: docs/plans/2026-02-01-wesichain-v0-design.md
- Implementation: pending
