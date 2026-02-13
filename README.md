<p align="center">
  <img src="https://raw.githubusercontent.com/wesichain/wesichain/main/assets/logo.svg" alt="Wesichain" width="200">
</p>

<h1 align="center">Wesichain</h1>

<p align="center">
  <strong>Build production-grade LLM agents in Rust</strong><br>
  Composable chains · Resumable graph workflows · Streaming-first runtime
</p>

<p align="center">
  <a href="https://github.com/wesichain/wesichain/actions">
    <img src="https://github.com/wesichain/wesichain/workflows/CI/badge.svg" alt="CI Status">
  </a>
  <a href="https://crates.io/crates/wesichain-core">
    <img src="https://img.shields.io/crates/v/wesichain-core.svg" alt="wesichain-core">
  </a>
  <a href="https://crates.io/crates/wesichain-graph">
    <img src="https://img.shields.io/crates/v/wesichain-graph.svg" alt="wesichain-graph">
  </a>
  <a href="https://crates.io/crates/wesichain-rag">
    <img src="https://img.shields.io/crates/v/wesichain-rag.svg" alt="wesichain-rag">
  </a>
  <a href="https://docs.rs/wesichain-core">
    <img src="https://docs.rs/wesichain-core/badge.svg" alt="docs.rs wesichain-core">
  </a>
  <img src="https://img.shields.io/badge/rust-1.75+-orange.svg" alt="Rust 1.75+">
  <a href="./LICENSE-MIT">
    <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License">
  </a>
</p>

---

Wesichain `v0.1.0` is live on crates.io as a modular crate family.

- 15 published crates, each installable independently
- no umbrella `wesichain` crate yet (intentional for minimal dependency footprints)
- designed for Rust-native RAG, stateful graph execution, and tool-using agents

---

## Quick Start (Modular)

### 1) Add dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
wesichain-core = "0.1.0"
wesichain-rag = "0.1.0"
```

### 2) Minimal in-memory RAG flow

```rust
use std::collections::HashMap;

use wesichain_core::{Document, Value};
use wesichain_rag::{RagQueryRequest, WesichainRag};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rag = WesichainRag::builder().build()?;

    rag.add_documents(vec![Document {
        id: "doc-1".to_string(),
        content: "Rust focuses on safety, speed, and fearless concurrency.".to_string(),
        metadata: HashMap::<String, Value>::new(),
        embedding: None,
    }])
    .await?;

    let response = rag
        .query(RagQueryRequest {
            query: "What does Rust focus on?".to_string(),
            thread_id: None,
        })
        .await?;

    println!("{}", response.answer);
    Ok(())
}
```

For end-to-end examples (streaming, sqlite checkpoints, retriever graphs):

- `wesichain-rag/examples/simple-rag-stream.rs`
- `wesichain-graph/examples/persistent_conversation.rs`
- `wesichain-graph/examples/react_agent.rs`

---

## Wesichain Crates (v0.1.0)

Wesichain is modular by default; install only the crates you need.

| Crate | Purpose | crates.io | docs.rs |
|---|---|---|---|
| `wesichain-core` | Core traits and runtime primitives (`Runnable`, tools, parsers, vector-store traits) | [link](https://crates.io/crates/wesichain-core) | [link](https://docs.rs/wesichain-core) |
| `wesichain-prompt` | Prompt templates and prompt formatting utilities | [link](https://crates.io/crates/wesichain-prompt) | [link](https://docs.rs/wesichain-prompt) |
| `wesichain-llm` | LLM provider adapters and request/response abstractions | [link](https://crates.io/crates/wesichain-llm) | [link](https://docs.rs/wesichain-llm) |
| `wesichain-macros` | Procedural macros for ergonomic integration | [link](https://crates.io/crates/wesichain-macros) | [link](https://docs.rs/wesichain-macros) |
| `wesichain-embeddings` | Embedding interfaces and providers | [link](https://crates.io/crates/wesichain-embeddings) | [link](https://docs.rs/wesichain-embeddings) |
| `wesichain-retrieval` | Retrieval utilities (indexing, splitting, retrievers) | [link](https://crates.io/crates/wesichain-retrieval) | [link](https://docs.rs/wesichain-retrieval) |
| `wesichain-agent` | Agent orchestration and tool-calling flows | [link](https://crates.io/crates/wesichain-agent) | [link](https://docs.rs/wesichain-agent) |
| `wesichain-graph` | Stateful graph execution, routing, interrupts, and checkpoints | [link](https://crates.io/crates/wesichain-graph) | [link](https://docs.rs/wesichain-graph) |
| `wesichain-checkpoint-sql` | Shared SQL checkpoint schema/operations | [link](https://crates.io/crates/wesichain-checkpoint-sql) | [link](https://docs.rs/wesichain-checkpoint-sql) |
| `wesichain-checkpoint-sqlite` | SQLite checkpoint backend | [link](https://crates.io/crates/wesichain-checkpoint-sqlite) | [link](https://docs.rs/wesichain-checkpoint-sqlite) |
| `wesichain-checkpoint-postgres` | Postgres checkpoint backend | [link](https://crates.io/crates/wesichain-checkpoint-postgres) | [link](https://docs.rs/wesichain-checkpoint-postgres) |
| `wesichain-rag` | RAG pipeline helpers built on core + graph + retrieval | [link](https://crates.io/crates/wesichain-rag) | [link](https://docs.rs/wesichain-rag) |
| `wesichain-langsmith` | LangSmith-compatible tracing/observability integration | [link](https://crates.io/crates/wesichain-langsmith) | [link](https://docs.rs/wesichain-langsmith) |
| `wesichain-pinecone` | Pinecone vector store integration | [link](https://crates.io/crates/wesichain-pinecone) | [link](https://docs.rs/wesichain-pinecone) |
| `wesichain-compat` | Compatibility utilities for migration-oriented workflows | [link](https://crates.io/crates/wesichain-compat) | [link](https://docs.rs/wesichain-compat) |

---

## Installation Patterns

Use only what you need:

```toml
# Core chain primitives
[dependencies]
wesichain-core = "0.1.0"

# Add graph execution
wesichain-graph = "0.1.0"

# Add RAG utilities
wesichain-rag = "0.1.0"

# Add sqlite checkpoint backend
wesichain-checkpoint-sqlite = "0.1.0"
```

---

## Performance

| Metric | Python LangChain | Wesichain (Rust) | Improvement |
|---|---|---|---|
| Memory (baseline) | 250-500 MB | 80-150 MB | 3-5x lower |
| Cold start | 2-5s | 50-200ms | 10-50x faster |
| Throughput | GIL-limited | Native parallel | scales with cores |
| Latency p99 | GC spikes | Predictable | lower jitter |

Benchmark notes and methodology: `docs/benchmarks/README.md`.

---

## Documentation

| Resource | Description |
|---|---|
| [docs.rs (core)](https://docs.rs/wesichain-core) | API reference for core abstractions |
| [docs.rs (graph)](https://docs.rs/wesichain-graph) | Graph runtime API reference |
| [docs.rs (rag)](https://docs.rs/wesichain-rag) | RAG pipeline API reference |
| [Migration Guide](docs/migration/langgraph-to-wesichain.md) | LangGraph to Wesichain migration notes |
| [Examples](wesichain-rag/examples/) | Working RAG examples |
| [Design Docs](docs/plans/) | Architecture and implementation plans |

---

## Development

```bash
# Build all workspace crates
cargo build --all

# Run tests
cargo test --all

# Lint and format
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

---

## Contributing

Contributions are welcome. Start with `CONTRIBUTING.md`.

- [Open an issue](https://github.com/wesichain/wesichain/issues)
- [Submit a pull request](https://github.com/wesichain/wesichain/pulls)

---

## License

Wesichain is dual licensed:

- [MIT](LICENSE-MIT)
- [Apache-2.0](LICENSE-APACHE)

---

<p align="center">
  Built with Rust · Inspired by LangChain/LangGraph · Optimized for production<br>
  <a href="https://github.com/wesichain/wesichain">GitHub</a> ·
  <a href="https://crates.io/search?q=wesichain-">Crates.io</a> ·
  <a href="https://docs.rs/wesichain-core">Documentation</a>
</p>
