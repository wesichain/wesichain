# wesichain-server

Axum HTTP server for exposing Wesichain agents and chains as REST/streaming APIs.

Part of the [wesichain](https://github.com/wesichain/wesichain) LLM agent framework.

## Installation

```toml
[dependencies]
wesichain-server = "0.3"
```

## Quick Start

```rust
use wesichain_server::{AgentRouter, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig::builder()
        .bind("0.0.0.0:8080")
        .bearer_token(std::env::var("API_TOKEN").ok())
        .build();

    AgentRouter::new(config)
        .run()
        .await?;
    Ok(())
}
```

## Features

- **Bearer auth** — optional `Authorization: Bearer <token>` middleware
- **Rate limiting** — configurable request-per-second limits via Tower
- **SSE streaming** — `/invoke/stream` endpoint using `text/event-stream`
- **JSON invoke** — `/invoke` endpoint for synchronous calls
- **Body size limit** — configurable max request body size

## License

Apache-2.0 OR MIT
