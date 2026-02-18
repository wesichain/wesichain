# Wesichain AI Coding Guide – Strict Rules (MUST follow)

This document provides strict, mechanical rules for AI agents working with the Wesichain codebase. Follow these rules without deviation. When in doubt, refer to the Golden Rule.

---

## Golden Rule #0

**If a pattern/method is not shown in docs/skills/*.md or official examples, DO NOT invent it.**

- Only use patterns explicitly documented in skills files
- Only use methods shown in working code examples
- If a pattern is not documented, ask for clarification
- Never assume LangChain Python patterns translate directly to Wesichain Rust

---

## 8 Universal Rules

### 1. Always Prefer Builders

Builders are the canonical way to construct complex objects in Wesichain.

**CORRECT:**
```rust
use wesichain_graph::{GraphBuilder, StateSchema};

let graph = GraphBuilder::new()
    .add_node("agent", agent_node)
    .add_node("tool", tool_node)
    .add_edge(Edge::from("agent").to("tool"))
    .add_edge(Edge::from("tool").to("agent"))
    .build()
    .expect("valid graph");
```

**WRONG:**
```rust
// Do not construct graphs manually
let mut graph = Graph::new();
graph.nodes.push(agent_node);
graph.edges.push(Edge::new("agent", "tool"));
```

---

### 2. State Must Derive Required Traits

All state structs MUST derive the required traits for serialization and checkpointing.

**CORRECT:**
```rust
use wesichain_graph::StateSchema;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, StateSchema, Default)]
struct AgentState {
    messages: Vec<ChatMessage>,
    tool_calls: Vec<ToolCall>,
    final_answer: Option<String>,
}
```

**WRONG:**
```rust
// Missing required derives
#[derive(Debug)]
struct AgentState {
    messages: Vec<ChatMessage>,
}

// Do not implement StateSchema manually
impl StateSchema for AgentState {
    // manual implementation
}
```

---

### 3. Tools Return JSON Values

Tools must return `serde_json::Value` for compatibility with the agent system.

**CORRECT:**
```rust
use serde_json::json;
use wesichain_core::Tool;

#[derive(Debug, Clone)]
struct SearchTool;

#[async_trait]
impl Tool for SearchTool {
    async fn invoke(&self, input: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        let query = input["query"].as_str().ok_or(ToolError::InvalidInput)?;
        let results = self.perform_search(query).await?;
        Ok(json!({
            "results": results,
            "count": results.len()
        }))
    }
}
```

**WRONG:**
```rust
// Do not return custom structs or strings directly
async fn invoke(&self, input: serde_json::Value) -> Result<String, ToolError> {
    Ok("search results".to_string())
}

// Do not return raw bytes
async fn invoke(&self, input: serde_json::Value) -> Result<Vec<u8>, ToolError> {
    Ok(vec![1, 2, 3])
}
```

---

### 4. All Execution Is Async

Wesichain is built on Tokio. All I/O operations and LLM calls must be async.

**CORRECT:**
```rust
use wesichain_core::{Runnable, Chain};

#[async_trait]
impl Runnable<String, String> for MyChain {
    async fn invoke(&self, input: String) -> Result<String, ChainError> {
        let result = self.llm.generate(&input).await?;
        Ok(result)
    }
    
    async fn stream(&self, 
        input: String
    ) -> Result<BoxStream<Result<StreamEvent, ChainError>>, ChainError> {
        self.llm.stream(&input).await
    }
}
```

**WRONG:**
```rust
// Never block in async context
fn invoke(&self, input: String) -> Result<String, ChainError> {
    std::thread::sleep(Duration::from_secs(1)); // BLOCKS!
    Ok(result)
}

// Do not use blocking LLM calls
let response = reqwest::blocking::get(url)?; // WRONG
```

---

### 5. Agent Logic -> ALWAYS Use wesichain_graph

All agent workflows must be built using the graph abstraction in `wesichain_graph`.

**CORRECT:**
```rust
use wesichain_graph::{GraphBuilder, StateSchema, Node, Edge};
use wesichain_agent::{ReActAgent, AgentConfig};

let agent = ReActAgent::new(config);

let graph = GraphBuilder::new()
    .add_node("agent", Node::from_runnable(agent))
    .add_node("tools", Node::from_tools(tools))
    .add_conditional_edges("agent", |state| {
        if state.has_tool_calls() {
            "tools"
        } else {
            "__end__"
        }
    })
    .add_edge(Edge::from("tools").to("agent"))
    .build()?;
```

**WRONG:**
```rust
// Do not implement agent loops manually
loop {
    let response = llm.generate(prompt).await?;
    if let Some(tool_call) = parse_tool_call(&response) {
        let result = execute_tool(tool_call).await?;
        prompt.push_str(&format!("Result: {}", result));
    } else {
        break;
    }
}

// Do not use recursion for agent control flow
async fn agent_step(state: State, depth: usize) -> Result<State> {
    if depth > 10 { return Ok(state); }
    let new_state = step(state).await?;
    agent_step(new_state, depth + 1).await // WRONG
}
```

---

### 6. Never Block Tokio Runtime

Blocking operations must be spawned on a dedicated thread pool.

**CORRECT:**
```rust
use tokio::task;

// CPU-intensive work
let result = task::spawn_blocking(move || {
    expensive_computation(data)
}).await?;

// Sync I/O
let content = task::spawn_blocking(move || {
    std::fs::read_to_string(path)
}).await??;
```

**WRONG:**
```rust
// Never block the async runtime
let data = std::fs::read_to_string("file.txt")?; // BLOCKS!

// Never perform CPU-intensive work directly
let hashed = expensive_hash(data); // BLOCKS!

// Never use std::sync::Mutex in async code
let guard = mutex.lock().unwrap(); // May block!
```

---

### 7. Checkpointing for Production

Production agents must use checkpointing for resumability and fault tolerance.

**CORRECT:**
```rust
use wesichain_graph::{FileCheckpointer, Checkpointer};

let checkpointer = FileCheckpointer::new("./checkpoints")
    .with_compression(true);

let executor = GraphExecutor::new(graph)
    .with_checkpointer(checkpointer)
    .with_save_interval(5);

// State is automatically saved
let result = executor.run(initial_state).await?;
```

**WRONG:**
```rust
// Do not run production agents without checkpointing
let result = graph.run(initial_state).await?; // No checkpointing!

// Do not implement custom checkpointing logic
fn my_save_state(state: &State) {
    fs::write("state.json", serde_json::to_string(state).unwrap());
}
```

---

### 8. Error Handling Patterns

Use the specific error types provided by Wesichain crates.

**CORRECT:**
```rust
use wesichain_core::errors::{ChainError, ToolError};
use thiserror::Error;

#[derive(Error, Debug)]
enum MyAgentError {
    #[error("chain error: {0}")]
    Chain(#[from] ChainError),
    
    #[error("tool error: {0}")]
    Tool(#[from] ToolError),
    
    #[error("invalid state: {0}")]
    InvalidState(String),
}

// Propagate with ?
let result = chain.invoke(input).await?;
```

**WRONG:**
```rust
// Do not use anyhow in library code
use anyhow::Result;

fn do_something() -> Result<()> {
    // ...
}

// Do not use panics for error handling
if state.is_invalid() {
    panic!("invalid state!"); // WRONG
}

// Do not ignore errors
let _ = risky_operation(); // WRONG - error ignored
```

---

## Quick Pattern Reference

| Pattern | Use This | Never Use |
|---------|----------|-----------|
| Graph construction | `GraphBuilder` | Manual `Graph` struct manipulation |
| State definition | `#[derive(StateSchema)]` | Manual `StateSchema` impl |
| Tool return type | `serde_json::Value` | Custom structs, raw strings, bytes |
| Async execution | `#[async_trait]` + Tokio | Blocking std APIs |
| Agent control flow | `wesichain_graph` nodes/edges | Manual loops, recursion |
| Blocking operations | `tokio::task::spawn_blocking` | Direct blocking calls |
| Persistence | `FileCheckpointer` or custom `Checkpointer` | Manual file I/O |
| Errors | `thiserror` enums with `#[from]` | `anyhow`, `panic!`, ignored errors |
| LLM calls | Provider-agnostic `LLM` trait | Direct HTTP clients |
| Composition | `.then()`, `.with_retries()` | Manual chaining |

---

## AI Prompt Booster

When working with Wesichain, prefix your prompts with:

```
You are working with Wesichain, a Rust-native LLM framework.

CRITICAL RULES:
1. ONLY use patterns from docs/skills/*.md files
2. ALWAYS use GraphBuilder for agent workflows
3. State MUST derive StateSchema, Serialize, Deserialize
4. Tools MUST return serde_json::Value
5. ALL execution is async using Tokio
6. NEVER block the runtime - use spawn_blocking for sync operations
7. ALWAYS use checkpointing for production agents
8. Use thiserror for errors, never anyhow or panics

If a pattern is not documented, STOP and ask for clarification.
Do not invent patterns based on Python LangChain knowledge.
```

---

## File Organization

Skills documentation is organized by topic:

| File | Topic |
|------|-------|
| `docs/skills/00-overview.md` | Framework overview and philosophy |
| `docs/skills/01-runnables.md` | Runnable trait and composition |
| `docs/skills/02-chains.md` | Building chains with LCEL |
| `docs/skills/03-tools.md` | Tool definition and usage |
| `docs/skills/04-agents.md` | ReAct agent implementation |
| `docs/skills/05-graphs.md` | Graph construction and execution |
| `docs/skills/06-state.md` | State management and checkpointing |
| `docs/skills/07-llms.md` | LLM provider configuration |
| `docs/skills/08-callbacks.md` | Observability and callbacks |
| `docs/skills/09-testing.md` | Testing patterns |
| `docs/skills/10-deployment.md` | Production deployment |

---

## Common Pitfalls to Avoid

1. **Assuming Python LangChain patterns work identically**
   - Wesichain is Rust-native with different ownership and lifetime rules
   - Composition patterns are similar but not identical

2. **Forgetting Send + Sync bounds**
   - All types crossing async boundaries must be Send + Sync
   - Use `#[derive(Clone)]` liberally for state

3. **Blocking the runtime**
   - Any blocking operation kills throughput
   - Always use `spawn_blocking` for sync work

4. **Manual state serialization**
   - Always use the derive macro for StateSchema
   - Manual implementations are error-prone

5. **Skipping checkpointing in production**
   - Without checkpointing, agents cannot resume after failures
   - Always configure a checkpointer for production

6. **Using std::sync types in async**
   - Use `tokio::sync::Mutex` and `tokio::sync::RwLock`
   - Never use `std::sync::Mutex` across await points

7. **Ignoring backpressure**
   - Use bounded channels for streaming
   - Handle slow consumers properly

8. **Over-complicating graph structures**
   - Start simple with linear chains
   - Add conditional edges only when necessary

---

## Resources

- **Repository**: `/Users/bene/Documents/bene/python/rechain/wesichain`
- **Design Plans**: `wesichain/docs/plans/` (v0 architecture decisions)
- **API Documentation**: Run `cargo doc --open` in the wesichain directory
- **Examples**: Check `wesichain-*/examples/` directories in each crate
- **Tests**: Reference tests in `wesichain-*/tests/` for working patterns
- **Roadmap**: See `wesichain/ROADMAP.md` for upcoming features

---

**Remember: When in doubt, consult the skills files. Never invent undocumented patterns.**
