<p align="center">
  <img src="https://raw.githubusercontent.com/wesichain/wesichain/main/assets/logo.svg" alt="Wesichain" width="200">
</p>

<h1 align="center">Wesichain</h1>

<p align="center">
  <strong>Build production-grade LLM agents in Rust</strong><br>
  Composable chains · Resumable graph workflows · Streaming-first runtime
</p>

<p align="center">
  Python parity informs ergonomics, Rust strengths drive correctness model.
</p>

<p align="center">
  <a href="https://wesichain.vercel.app/">
    <img src="https://img.shields.io/badge/docs-wesichain.vercel.app-orange?style=flat" alt="Documentation">
  </a>
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

🚀 **[Get Started →](https://wesichain.vercel.app/docs/getting-started/installation)** · **[Explore Crates →](https://wesichain.vercel.app/crate-selector)** · **[View Benchmarks →](https://wesichain.vercel.app/benchmarks)**

---

Wesichain `v0.2.0` is live on crates.io as a modular crate family, with the v0.3 agent runtime track in active implementation.

- 19 published crates, each installable independently
- no umbrella `wesichain` crate on crates.io yet (intentional for minimal dependency footprints)
- designed for tool-using ReAct agents, stateful graph execution, and RAG
- **Note:** `wesichain-agent` is the FSM-first runtime track for v0.3 and is currently under active implementation in this repository.

---

## Quick Start (ReAct First)

### 1) Add dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
wesichain-core = "0.2.0"
wesichain-graph = "0.2.0"
wesichain-llm = "0.2.0"
```

### 2) Create a ReAct agent with tools

```rust
use std::sync::Arc;

use wesichain_core::{HasFinalOutput, HasUserInput, ScratchpadState, ToolCallingLlm};
use wesichain_graph::{GraphBuilder, GraphState, ReActAgentNode, StateSchema};

// AppState implements:
// StateSchema + ScratchpadState + HasUserInput + HasFinalOutput
let llm: Arc<dyn ToolCallingLlm> = Arc::new(my_llm);

let react_node = ReActAgentNode::builder()
    .llm(llm)
    .tools(vec![Arc::new(CalculatorTool), Arc::new(SearchTool)])
    .max_iterations(12)
    .build()?;

let graph = GraphBuilder::new()
    .add_node("agent", react_node)
    .set_entry("agent")
    .build();

let initial = GraphState::new(AppState::from_input("Find 2+2, then explain it."));
let result = graph.invoke_graph(initial).await?;
println!("{:?}", result.data);
```

For full runnable ReAct examples:

- `cargo run -p wesichain-graph --example react_agent`
- `cargo run -p wesichain-graph --example persistent_conversation`

### 3) Add RAG when you need retrieval grounding

```rust
use wesichain_rag::{RagQueryRequest, WesichainRag};

let rag = WesichainRag::builder().build()?;
let response = rag
    .query(RagQueryRequest {
        query: "What does Rust focus on?".to_string(),
        thread_id: None,
    })
    .await?;
println!("{}", response.answer);
```

For end-to-end RAG streaming example:

- `cargo run -p wesichain-rag --example simple-rag-stream`

### 4) Pick the right starting point

| If you need | Start with |
|---|---|
| Tool use + multi-step reasoning | ReAct graph agent (`wesichain-graph` + `ReActAgentNode`) |
| Retrieval-grounded answers | `wesichain-rag` |
| Both | ReAct graph for orchestration + retrieval as a node/tool |

---

## Wesichain Crates (v0.2.0)

Wesichain is modular by default; install only the crates you need.

| Crate | Purpose | crates.io | docs.rs |
|---|---|---|---|
| `wesichain-core` | Core traits and runtime primitives (`Runnable`, tools, parsers, vector-store traits) | [link](https://crates.io/crates/wesichain-core) | [link](https://docs.rs/wesichain-core) |
| `wesichain-prompt` | Prompt templates and prompt formatting utilities | [link](https://crates.io/crates/wesichain-prompt) | [link](https://docs.rs/wesichain-prompt) |
| `wesichain-llm` | LLM provider adapters and request/response abstractions | [link](https://crates.io/crates/wesichain-llm) | [link](https://docs.rs/wesichain-llm) |
| `wesichain-macros` | Procedural macros for ergonomic integration | [link](https://crates.io/crates/wesichain-macros) | [link](https://docs.rs/wesichain-macros) |
| `wesichain-embeddings` | Embedding interfaces and providers | [link](https://crates.io/crates/wesichain-embeddings) | [link](https://docs.rs/wesichain-embeddings) |
| `wesichain-retrieval` | Retrieval utilities (indexing, splitting, retrievers) | [link](https://crates.io/crates/wesichain-retrieval) | [link](https://docs.rs/wesichain-retrieval) |

| `wesichain-graph` | Stateful graph execution, routing, interrupts, and checkpoints | [link](https://crates.io/crates/wesichain-graph) | [link](https://docs.rs/wesichain-graph) |
| `wesichain-checkpoint-sql` | Shared SQL checkpoint schema/operations | [link](https://crates.io/crates/wesichain-checkpoint-sql) | [link](https://docs.rs/wesichain-checkpoint-sql) |
| `wesichain-checkpoint-sqlite` | SQLite checkpoint backend | [link](https://crates.io/crates/wesichain-checkpoint-sqlite) | [link](https://docs.rs/wesichain-checkpoint-sqlite) |
| `wesichain-checkpoint-postgres` | Postgres checkpoint backend | [link](https://crates.io/crates/wesichain-checkpoint-postgres) | [link](https://docs.rs/wesichain-checkpoint-postgres) |
| `wesichain-checkpoint-redis` | Redis checkpoint backend | [link](https://crates.io/crates/wesichain-checkpoint-redis) | [link](https://docs.rs/wesichain-checkpoint-redis) |
| `wesichain-rag` | RAG pipeline helpers built on core + graph + retrieval | [link](https://crates.io/crates/wesichain-rag) | [link](https://docs.rs/wesichain-rag) |
| `wesichain-langsmith` | LangSmith-compatible tracing/observability integration | [link](https://crates.io/crates/wesichain-langsmith) | [link](https://docs.rs/wesichain-langsmith) |
| `wesichain-chroma` | Chroma vector store integration | [link](https://crates.io/crates/wesichain-chroma) | [link](https://docs.rs/wesichain-chroma) |
| `wesichain-pinecone` | Pinecone vector store integration | [link](https://crates.io/crates/wesichain-pinecone) | [link](https://docs.rs/wesichain-pinecone) |
| `wesichain-qdrant` | Qdrant vector store integration | [link](https://crates.io/crates/wesichain-qdrant) | [link](https://docs.rs/wesichain-qdrant) |
| `wesichain-weaviate` | Weaviate vector store integration | [link](https://crates.io/crates/wesichain-weaviate) | [link](https://docs.rs/wesichain-weaviate) |
| `wesichain-compat` | Compatibility utilities for migration-oriented workflows | [link](https://crates.io/crates/wesichain-compat) | [link](https://docs.rs/wesichain-compat) |

---

## Installation Patterns

Use only what you need:

```toml
# Core chain primitives
[dependencies]
wesichain-core = "0.2.0"

# Add graph execution
wesichain-graph = "0.2.0"

# Add RAG utilities
wesichain-rag = "0.2.0"

# Add sqlite checkpoint backend
wesichain-checkpoint-sqlite = "0.2.0"
```

---

## Performance

| Metric | Typical Python baseline | Wesichain (Rust) | Improvement |
|---|---|---|---|
| Memory (baseline) | 250-500 MB | 80-150 MB | 3-5x lower |
| Cold start | 2-5s | 50-200ms | 10-50x faster |
| Throughput | GIL-limited | Native parallel | scales with cores |
| Latency p99 | GC spikes | Predictable | lower jitter |

Benchmark notes and methodology: `docs/benchmarks/README.md`.

---

## Documentation

📖 **[Official Documentation Site →](https://wesichain.vercel.app/)**

| Resource | Description |
|---|---|
| [docs.rs (core)](https://docs.rs/wesichain-core) | API reference for core abstractions |
| [docs.rs (graph)](https://docs.rs/wesichain-graph) | Graph runtime API reference |
| [docs.rs (rag)](https://docs.rs/wesichain-rag) | RAG pipeline API reference |
| [Migration Guide](https://wesichain.vercel.app/docs/getting-started/migration) | Graph workflow migration notes |
| [Examples (ReAct + Graph)](wesichain-graph/examples/) | ReAct and graph workflow examples |
| [Examples (RAG)](wesichain-rag/examples/) | Retrieval and streaming RAG examples |
| [Design Docs](docs/plans/) | Architecture and implementation plans |

---

## AI-Assisted Development

This repository includes AI-optimized documentation for coding agents:

- **[docs/AGENTS.md](docs/AGENTS.md)** - Universal coding rules for AI agents
- **[docs/skills/](docs/skills/)** - Detailed patterns for RAG, agents, and core concepts
- **`.claude/skills/`** - Claude Code-optimized skills (auto-discovered)

### Quick Start for AI Agents

1. Read `docs/AGENTS.md` first (strict rules)
2. Reference `docs/skills/*.md` for specific patterns
3. Use builder patterns, async/await, and proper error handling
4. Never invent methods - only use patterns shown in docs

See [docs/skills/README.md](docs/skills/README.md) for the complete guide index.

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
  Built with Rust · Designed for production graph workflows<br>
  <a href="https://wesichain.vercel.app/">📖 Documentation</a> ·
  <a href="https://github.com/wesichain/wesichain">GitHub</a> ·
  <a href="https://crates.io/search?q=wesichain-">Crates.io</a>
</p>
