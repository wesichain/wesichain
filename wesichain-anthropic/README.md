# wesichain-anthropic

Anthropic Claude provider for Wesichain — streaming, tool use, and extended thinking support.

Part of the [wesichain](https://github.com/wesichain/wesichain) LLM agent framework.

## Installation

```toml
[dependencies]
wesichain-anthropic = "0.3"
```

## Quick Start

```rust
use wesichain_anthropic::AnthropicLlm;
use wesichain_core::runnable::Runnable;
use wesichain_core::llm::LlmRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm = AnthropicLlm::from_env(); // reads ANTHROPIC_API_KEY

    let request = LlmRequest::user("What is the capital of France?");
    let response = llm.invoke(request).await?;
    println!("{}", response.content);
    Ok(())
}
```

## Features

| Flag | Description |
|------|-------------|
| `stream` (default) | Server-sent event streaming |
| `vision` | Image input support |

### Extended Thinking

```rust
let llm = AnthropicLlm::builder()
    .model("claude-opus-4-5")
    .thinking_budget(8000)
    .build();
```

## License

Apache-2.0 OR MIT
