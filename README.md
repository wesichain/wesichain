<p align="center">
  <img src="https://raw.githubusercontent.com/wesichain/wesichain/main/assets/logo.svg" alt="Wesichain" width="200">
</p>

<h1 align="center">Wesichain</h1>

<p align="center">
  <strong>Build production-grade LLM agents in Rust</strong><br>
  Composable chains · Resumable ReAct agents · 30-70% lower memory
</p>

<p align="center">
  <a href="https://github.com/wesichain/wesichain/actions">
    <img src="https://github.com/wesichain/wesichain/workflows/CI/badge.svg" alt="CI Status">
  </a>
  <a href="https://crates.io/crates/wesichain">
    <img src="https://img.shields.io/crates/v/wesichain.svg" alt="Crates.io">
  </a>
  <a href="https://docs.rs/wesichain">
    <img src="https://docs.rs/wesichain/badge.svg" alt="Documentation">
  </a>
  <img src="https://img.shields.io/badge/rust-1.75+-orange.svg" alt="Rust 1.75+">
  <a href="./LICENSE-MIT">
    <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License">
  </a>
</p>

---

## ✨ Features

<table>
<tr>
<td width="33%">

**🧩 Composable Chains**

Build pipelines with intuitive `.then()` composition. Familiar API for LangChain developers.

</td>
<td width="33%">

**🔄 Resumable Agents**

ReAct agents with automatic checkpointing. Resume workflows after crashes or restarts.

</td>
<td width="33%">

**⚡ Streaming First**

Token-by-token streaming with structured events. Real-time responses for better UX.

</td>
</tr>
<tr>
<td width="33%">

**🕸️ Graph Workflows**

LangGraph-style state machines with cycles, branches, and parallel execution.

</td>
<td width="33%">

**🔌 Provider Agnostic**

OpenAI, Anthropic, Google Gemini, Ollama, Mistral. Switch providers with one line.

</td>
<td width="33%">

**📊 Built-in Observability**

LangSmith integration for tracing, debugging, and monitoring agent execution.

</td>
</tr>
</table>

---

## 🚀 Quick Start

### 1. Add to Cargo.toml

```toml
[dependencies]
wesichain = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

### 2. Build your first chain

```rust
use wesichain_core::{Runnable, RunnableExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compose a chain: Prompt → LLM → Parser
    let chain = Prompt
        .then(OpenAiClient::new("gpt-4o-mini"))
        .then(JsonParser);

    // Run it
    let result = chain
        .invoke("What is Rust's ownership model?".to_string())
        .await?;

    println!("{}", result);
    Ok(())
}
```

### 3. Add a ReAct agent with tools

```rust
use wesichain_agent::{ReActAgent, Tool};

let agent = ReActAgent::builder()
    .llm(openai_client)
    .tools(vec![
        Calculator,
        WebSearch,
        CodeInterpreter,
    ])
    .checkpoint(JsonFileCheckpointer::new("./checkpoints"))
    .build()?;

// The agent can use tools and resumes if interrupted
let result = agent
    .invoke("Calculate fibonacci(50) and search for its significance")
    .await?;
```

---

## 📦 Installation

### With specific providers

```toml
# OpenAI only
[dependencies]
wesichain = { version = "0.1", features = ["openai"] }

# Multiple providers
[dependencies]
wesichain = { version = "0.1", features = ["openai", "anthropic", "ollama"] }

# All features (including postgres checkpointing)
[dependencies]
wesichain = { version = "0.1", features = ["full"] }
```

### Available features

| Feature | Description |
|---------|-------------|
| `openai` | OpenAI GPT models |
| `anthropic` | Anthropic Claude models |
| `google` | Google Gemini models |
| `ollama` | Local Ollama models |
| `mistral` | Mistral AI models |
| `postgres` | Postgres checkpointing |
| `sqlite` | SQLite checkpointing |
| `langsmith` | Observability integration |
| `full` | All features enabled |

---

## 🏗️ Architecture

Wesichain is a workspace of focused, composable crates:

```
wesichain/
├── wesichain-core          # Core traits: Runnable, Chain, Tool, Checkpointer
├── wesichain-prompt        # Prompt templates with variable substitution
├── wesichain-llm           # Provider-agnostic LLM trait + adapters
├── wesichain-agent         # ReAct agent with memory and tool calling
├── wesichain-graph         # Stateful graph execution with persistence
├── wesichain-embeddings    # Text embedding models
├── wesichain-rag           # Retrieval-Augmented Generation
├── wesichain-retrieval     # Vector store integrations (Pinecone, Qdrant)
├── wesichain-langsmith     # Tracing and observability
└── wesichain               # Umbrella crate with ergonomic prelude
```

---

## 📊 Performance

| Metric | Python LangChain | Wesichain (Rust) | Improvement |
|--------|------------------|------------------|-------------|
| **Memory (baseline)** | 250-500 MB | 80-150 MB | **3-5x lower** |
| **Cold start** | 2-5s | 50-200ms | **10-50x faster** |
| **Throughput** | GIL-limited | Native parallel | **Unlimited scaling** |
| **Latency p99** | GC spikes | Predictable | **Zero pauses** |

*Benchmarks: 100 concurrent agent requests, 10 tool calls each. [Reproduce](./wesichain/benches/)*

---

## 📚 Documentation

| Resource | Description |
|----------|-------------|
| [API Reference](https://docs.rs/wesichain) | Complete API documentation |
| [Examples](./wesichain/examples/) | Working code examples |
| [Design Docs](./docs/plans/) | Architecture decisions |
| [Migration Guide](./docs/migration.md) | From LangChain/LangGraph |

---

## 🛠️ Development

```bash
# Clone the repository
git clone https://github.com/wesichain/wesichain.git
cd wesichain

# Build
cd wesichain && cargo build --release

# Run tests
cargo test
cargo test --features openai,postgres

# Run benchmarks
cargo bench

# Generate docs
cargo doc --open
```

---

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](./CONTRIBUTING.md) for details.

- [Report bugs](https://github.com/wesichain/wesichain/issues)
- [Request features](https://github.com/wesichain/wesichain/issues)
- [Submit PRs](https://github.com/wesichain/wesichain/pulls)

---

## 📄 License

Wesichain is dual-licensed under:

- [MIT License](./LICENSE-MIT)
- [Apache License 2.0](./LICENSE-APACHE)

You may use, distribute, and modify this software under either license at your option.

---

<p align="center">
  Built with Rust · Inspired by LangChain/LangGraph · Optimized for production<br>
  <a href="https://github.com/wesichain/wesichain">GitHub</a> ·
  <a href="https://crates.io/crates/wesichain">Crates.io</a> ·
  <a href="https://docs.rs/wesichain">Documentation</a>
</p>
