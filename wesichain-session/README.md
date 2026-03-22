# wesichain-session

Session persistence, cost tracking, and token budget management for Wesichain agents.

Part of the [wesichain](https://github.com/wesichain/wesichain) LLM agent framework.

## Installation

```toml
[dependencies]
wesichain-session = "0.3"
```

## Quick Start

```rust
use wesichain_session::{Session, FileSessionStore, SessionStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = FileSessionStore::new(".sessions");

    // Create or resume a session
    let mut session = store.load_or_create("my-agent-session").await?;

    // Track cost after an LLM call
    session.record_usage(1500, 300, 0.0025); // input_tokens, output_tokens, cost_usd

    store.save(&session).await?;
    println!("Total cost: ${:.4}", session.total_cost_usd());
    Ok(())
}
```

## Features

- **FileSessionStore** — persist sessions as JSON files
- **Cost tracking** — accumulate token counts and USD cost across turns
- **Token budget** — set hard limits to prevent runaway agents
- **Session ID** — UUID-based session identity for resumption

## License

Apache-2.0 OR MIT
