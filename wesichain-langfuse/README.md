# wesichain-langfuse

Langfuse observability callback handler for Wesichain — trace LLM calls, chains, and agent runs.

Part of the [wesichain](https://github.com/wesichain/wesichain) LLM agent framework.

## Installation

```toml
[dependencies]
wesichain-langfuse = "0.3"
```

## Quick Start

```rust
use wesichain_langfuse::LangfuseHandler;
use wesichain_core::callbacks::CallbackHandler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handler = LangfuseHandler::from_env()?;
    // reads LANGFUSE_PUBLIC_KEY, LANGFUSE_SECRET_KEY, LANGFUSE_HOST

    // Attach to a chain or agent
    // chain.with_callback(handler).invoke(input).await?
    Ok(())
}
```

## Features

- **Trace batching** — events are buffered and flushed in batches for efficiency
- **Span nesting** — chain and agent spans are properly nested as Langfuse traces
- **Cost recording** — token counts and estimated costs logged per generation
- **PII redaction** — optional redaction of sensitive fields before upload
- **Background flush** — non-blocking flush task with configurable interval

## License

Apache-2.0 OR MIT
