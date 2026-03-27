# Wesichain AI Coding Guide

Quick reference for agentic coding agents working with the Wesichain Rust codebase.

## Build, Lint, and Test Commands

```bash
# Build all crates
cargo build

# Build a specific crate
cargo build -p wesichain-core

# Run all tests
cargo test --all-features

# Run tests for a specific crate
cargo test -p wesichain-core

# Run a single test
cargo test -p wesichain-core test_name_here

# Run tests with output visible
cargo test -p wesichain-core -- --nocapture

# Format code
cargo fmt

# Check formatting without changes
cargo fmt -- --check

# Run clippy lints
cargo clippy --all-features -- -D warnings

# Generate documentation
cargo doc --open

# Run benchmarks
cargo bench -p wesichain-qdrant
```

## Code Style Guidelines

### Imports
- Group imports: std, external crates, internal crate
- Use `use crate::` for internal imports, not relative paths
- Alphabetically sort within groups

```rust
use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

use crate::{WesichainError, Runnable};
```

### Formatting
- Use `cargo fmt` with default settings (no custom rustfmt.toml)
- Maximum line length: implicit default (100 chars)
- 4 spaces for indentation

### Types and Naming
- Types: PascalCase (`GraphBuilder`, `ReActAgent`)
- Functions/methods: snake_case (`invoke`, `add_node`)
- Constants: SCREAMING_SNAKE_CASE (`START`, `END`)
- Traits: PascalCase with clear action names (`Runnable`, `Tool`)

### Error Handling
- Use `thiserror::Error` derive for error enums
- Never use `anyhow` in library code
- Propagate errors with `?` operator
- Provide context in error messages

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("node not found: {0}")]
    NodeNotFound(String),
    #[error("invalid edge: {from} -> {to}")]
    InvalidEdge { from: String, to: String },
    #[error(transparent)]
    Other(#[from] WesichainError),
}
```

### Async Patterns
- All I/O operations must be async using Tokio
- Use `#[async_trait]` for trait implementations
- Never block the runtime - use `tokio::task::spawn_blocking` for sync operations
- Return `BoxStream` for streaming interfaces

```rust
use async_trait::async_trait;
use futures::stream::BoxStream;

#[async_trait]
pub trait Runnable<Input, Output>: Send + Sync {
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError>;
    fn stream(&self, input: Input) -> BoxStream<Result<StreamEvent, WesichainError>>;
}
```

### State Management
- State structs must derive: `Debug, Clone, Serialize, Deserialize, StateSchema`
- Use `Default` derive for initialization
- Never implement `StateSchema` manually

```rust
#[derive(Debug, Clone, Serialize, Deserialize, StateSchema, Default)]
struct AgentState {
    messages: Vec<ChatMessage>,
    tool_calls: Vec<ToolCall>,
    final_answer: Option<String>,
}
```

### Tool Implementation
- Tools must return `serde_json::Value`
- Use `#[async_trait]` for `Tool` trait
- Parse input with proper error handling

```rust
use async_trait::async_trait;
use serde_json::json;
use wesichain_core::{Tool, ToolError};

#[derive(Debug, Clone)]
struct SearchTool;

#[async_trait]
impl Tool for SearchTool {
    async fn invoke(&self, input: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        let query = input["query"].as_str()
            .ok_or_else(|| ToolError::InvalidInput("missing query".into()))?;
        let results = self.search(query).await?;
        Ok(json!({ "results": results }))
    }
}
```

### Graph Construction
- Always use `GraphBuilder` for constructing graphs
- Use builder pattern methods chained together
- Never manipulate `Graph` struct directly

```rust
use wesichain_graph::{GraphBuilder, Edge};

let graph = GraphBuilder::new()
    .add_node("agent", agent_node)
    .add_node("tools", tool_node)
    .add_edge(Edge::from("agent").to("tools"))
    .build()?;
```

### Testing
- Write unit tests in `#[cfg(test)]` modules
- Use descriptive test names: `test_descriptive_name`
- Test both success and error cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_invoke_returns_expected() {
        let chain = setup_chain();
        let result = chain.invoke("input".to_string()).await.unwrap();
        assert_eq!(result, "expected");
    }
}
```

## Golden Rules

1. **Only use patterns from `docs/skills/*.md` files** - never invent undocumented patterns
2. **Always use `GraphBuilder`** for agent workflows
3. **State MUST derive** `StateSchema`, `Serialize`, `Deserialize`
4. **Tools MUST return** `serde_json::Value`
5. **ALL execution is async** using Tokio
6. **NEVER block the runtime** - use `spawn_blocking` for sync operations
7. **Use checkpointing** for production agents with `FileCheckpointer`
8. **Use `thiserror`** for errors, never `anyhow` or `panic!`

## Project Structure

- Workspace with 20+ crates: `wesichain-*`
- Core crates: `wesichain-core`, `wesichain-graph`, `wesichain-agent`
- Integration crates: `wesichain-qdrant`, `wesichain-chroma`, etc.
- Examples in `examples/` directory
- Skills documentation in `docs/skills/`

## Resources

- Repository: `/Users/bene/Documents/bene/python/rechain/wesichain`
- Skills: `docs/skills/*.md` - follow these patterns
- CI: `.github/workflows/ci.yml` for validation commands
- MSRV: Rust 1.75+
