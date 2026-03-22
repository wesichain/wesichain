# wesichain

Rust-native LLM agents & chains with resumable ReAct workflows — batteries-included facade crate.

Part of the [wesichain](https://github.com/wesichain/wesichain) LLM agent framework.

## Installation

```toml
[dependencies]
# Default: core + openai + react agent
wesichain = "0.3"

# Everything
wesichain = { version = "0.3", features = ["full"] }

# Coding agent stack (tools + agent + session + mcp)
wesichain = { version = "0.3", features = ["coding"] }
```

## Feature Flags

| Feature | Includes |
|---------|----------|
| `openai` | `wesichain-llm` OpenAI/Ollama/Groq providers |
| `anthropic` | `wesichain-anthropic` Claude client |
| `agent` | `wesichain-agent` ReAct agent + FSM |
| `graph` | `wesichain-graph` stateful execution graph |
| `tools` | `wesichain-tools` coding tools (fs, bash, git) |
| `server` | `wesichain-server` Axum HTTP API server |
| `mcp` | `wesichain-mcp` Model Context Protocol client |
| `session` | `wesichain-session` cost/token tracking |
| `memory` | `wesichain-memory` conversation memory |
| `retrieval` | `wesichain-retrieval` RAG retrieval |
| `langsmith` | `wesichain-langsmith` LangSmith observability |
| `langfuse` | `wesichain-langfuse` Langfuse observability |
| `qdrant` | `wesichain-qdrant` Qdrant vector store |
| `coding` | `tools` + `agent` + `session` + `mcp` |
| `full` | All features |

## Quick Start

```rust
use wesichain::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chain = Prompt::new("Answer briefly: {question}")
        .then(OpenAiLlm::from_env())
        .then(StringOutputParser);

    let answer = chain.invoke("What is Rust?".to_string()).await?;
    println!("{answer}");
    Ok(())
}
```

## License

Apache-2.0 OR MIT
