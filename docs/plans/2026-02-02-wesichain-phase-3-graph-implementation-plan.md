# Wesichain Phase 3 Graph + Checkpointing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a minimal LangGraph-style graph engine with typed state, conditional routing, and checkpointing in a new `wesichain-graph` crate.

**Architecture:** Introduce a `GraphBuilder` that compiles to `ExecutableGraph` with nodes as `Runnable<State, StateUpdate>`. State is a typed struct `S: StateSchema` (serde + clone). Execution applies per-node updates with simple merge rules and optional checkpointing after each node. Use petgraph for internal node/edge representation and tokio for async execution.

**Tech Stack:** Rust 1.75+, tokio, async-trait, serde, serde_json, thiserror, petgraph, uuid.

---

### Task 1: Add wesichain-graph crate and state types

**Files:**
- Create: `wesichain-graph/Cargo.toml`
- Create: `wesichain-graph/src/lib.rs`
- Create: `wesichain-graph/src/state.rs`
- Modify: `Cargo.toml`
- Test: `wesichain-graph/tests/state.rs`

**Step 1: Write the failing test**

```rust
use serde::{Deserialize, Serialize};
use wesichain_graph::{GraphState, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

#[test]
fn state_update_merges_last_write() {
    let base = GraphState::new(DemoState { count: 1 });
    let update = StateUpdate::new(DemoState { count: 2 });
    let merged = base.apply(update);
    assert_eq!(merged.data.count, 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test state -v`
Expected: FAIL with missing types.

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/state.rs
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub trait StateSchema: Serialize + DeserializeOwned + Clone + Default + Send + Sync + 'static {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphState<S: StateSchema> {
    pub data: S,
}

impl<S: StateSchema> GraphState<S> {
    pub fn new(data: S) -> Self {
        Self { data }
    }

    pub fn apply(self, update: StateUpdate<S>) -> Self {
        Self { data: update.data }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateUpdate<S: StateSchema> {
    pub data: S,
}

impl<S: StateSchema> StateUpdate<S> {
    pub fn new(data: S) -> Self {
        Self { data }
    }
}
```

```rust
// wesichain-graph/src/lib.rs
mod state;

pub use state::{GraphState, StateSchema, StateUpdate};
```

```toml
# wesichain-graph/Cargo.toml
[package]
name = "wesichain-graph"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
repository.workspace = true
homepage.workspace = true
description.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```toml
# Cargo.toml
[workspace]
members = [
  "wesichain",
  "wesichain-core",
  "wesichain-prompt",
  "wesichain-llm",
  "wesichain-agent",
  "wesichain-graph",
]
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test state -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/Cargo.toml wesichain-graph/src/lib.rs wesichain-graph/src/state.rs Cargo.toml wesichain-graph/tests/state.rs
git commit -m "feat(graph): add state types"
```

---

### Task 2: Define node and graph builder basics

**Files:**
- Create: `wesichain-graph/src/graph.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Modify: `wesichain-graph/Cargo.toml`
- Test: `wesichain-graph/tests/graph_builder.rs`

**Step 1: Write the failing test**

```rust
use wesichain_graph::{GraphBuilder, GraphState, StateSchema, StateUpdate};
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, WesichainError, StreamEvent};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState { count: i32 }

impl StateSchema for DemoState {}

struct AddOne;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for AddOne {
    async fn invoke(&self, input: GraphState<DemoState>) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState { count: input.data.count + 1 }))
    }

    fn stream(&self, _input: GraphState<DemoState>) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
async fn graph_builder_compiles_single_node() {
    let graph = GraphBuilder::new()
        .add_node("add", AddOne)
        .set_entry("add")
        .build();

    let state = GraphState::new(DemoState { count: 1 });
    let out = graph.invoke(state).await.unwrap();
    assert_eq!(out.data.count, 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test graph_builder -v`
Expected: FAIL with missing `GraphBuilder`.

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/graph.rs
use std::collections::HashMap;

use wesichain_core::{Runnable, WesichainError};
use crate::{GraphState, StateSchema, StateUpdate};

pub struct GraphBuilder<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    entry: Option<String>,
}

impl<S: StateSchema> GraphBuilder<S> {
    pub fn new() -> Self {
        Self { nodes: HashMap::new(), entry: None }
    }

    pub fn add_node<R>(mut self, name: &str, node: R) -> Self
    where
        R: Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync + 'static,
    {
        self.nodes.insert(name.to_string(), Box::new(node));
        self
    }

    pub fn set_entry(mut self, name: &str) -> Self {
        self.entry = Some(name.to_string());
        self
    }

    pub fn build(self) -> ExecutableGraph<S> {
        ExecutableGraph { nodes: self.nodes, entry: self.entry.expect("entry") }
    }
}

pub struct ExecutableGraph<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    entry: String,
}

impl<S: StateSchema> ExecutableGraph<S> {
    pub async fn invoke(&self, state: GraphState<S>) -> Result<GraphState<S>, WesichainError> {
        let node = self.nodes.get(&self.entry).expect("entry node");
        let update = node.invoke(state).await?;
        Ok(GraphState::new(update.data))
    }
}
```

```rust
// wesichain-graph/src/lib.rs
mod graph;

pub use graph::{ExecutableGraph, GraphBuilder};
```

```toml
# wesichain-graph/Cargo.toml
[dependencies]
async-trait = "0.1"
futures = "0.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
wesichain-core = { path = "../wesichain-core" }
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test graph_builder -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/graph.rs wesichain-graph/src/lib.rs wesichain-graph/Cargo.toml wesichain-graph/tests/graph_builder.rs
git commit -m "feat(graph): add graph builder"
```

---

### Task 3: Add edges and sequential execution

**Files:**
- Modify: `wesichain-graph/src/graph.rs`
- Test: `wesichain-graph/tests/graph_edges.rs`

**Step 1: Write the failing test**

```rust
use wesichain_graph::{GraphBuilder, GraphState, StateSchema, StateUpdate};
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, WesichainError, StreamEvent};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState { count: i32 }

impl StateSchema for DemoState {}

struct AddOne;
struct AddTwo;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for AddOne {
    async fn invoke(&self, input: GraphState<DemoState>) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState { count: input.data.count + 1 }))
    }
    fn stream(&self, _input: GraphState<DemoState>) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for AddTwo {
    async fn invoke(&self, input: GraphState<DemoState>) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState { count: input.data.count + 2 }))
    }
    fn stream(&self, _input: GraphState<DemoState>) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
async fn graph_executes_edges_in_order() {
    let graph = GraphBuilder::new()
        .add_node("one", AddOne)
        .add_node("two", AddTwo)
        .add_edge("one", "two")
        .set_entry("one")
        .build();

    let state = GraphState::new(DemoState { count: 1 });
    let out = graph.invoke(state).await.unwrap();
    assert_eq!(out.data.count, 4);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test graph_edges -v`
Expected: FAIL with missing `add_edge` or sequential execution.

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/graph.rs
pub struct GraphBuilder<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    edges: HashMap<String, String>,
    entry: Option<String>,
}

impl<S: StateSchema> GraphBuilder<S> {
    pub fn add_edge(mut self, from: &str, to: &str) -> Self {
        self.edges.insert(from.to_string(), to.to_string());
        self
    }

    pub fn build(self) -> ExecutableGraph<S> {
        ExecutableGraph {
            nodes: self.nodes,
            edges: self.edges,
            entry: self.entry.expect("entry"),
        }
    }
}

pub struct ExecutableGraph<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    edges: HashMap<String, String>,
    entry: String,
}

impl<S: StateSchema> ExecutableGraph<S> {
    pub async fn invoke(&self, mut state: GraphState<S>) -> Result<GraphState<S>, WesichainError> {
        let mut current = self.entry.clone();
        loop {
            let node = self.nodes.get(&current).expect("node");
            let update = node.invoke(state).await?;
            state = GraphState::new(update.data);
            match self.edges.get(&current) {
                Some(next) => current = next.clone(),
                None => break,
            }
        }
        Ok(state)
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test graph_edges -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/graph.rs wesichain-graph/tests/graph_edges.rs
git commit -m "feat(graph): add sequential edges"
```

---

### Task 4: Add conditional edges

**Files:**
- Modify: `wesichain-graph/src/graph.rs`
- Test: `wesichain-graph/tests/graph_conditional.rs`

**Step 1: Write the failing test**

```rust
use wesichain_graph::{GraphBuilder, GraphState, StateSchema, StateUpdate};
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, WesichainError, StreamEvent};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState { count: i32 }

impl StateSchema for DemoState {}

struct Inc;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for Inc {
    async fn invoke(&self, input: GraphState<DemoState>) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState { count: input.data.count + 1 }))
    }
    fn stream(&self, _input: GraphState<DemoState>) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
async fn graph_conditional_routes_by_state() {
    let graph = GraphBuilder::new()
        .add_node("inc", Inc)
        .add_node("inc2", Inc)
        .add_node("stop", Inc)
        .add_conditional_edge("inc", |state: &GraphState<DemoState>| {
            if state.data.count > 1 { "stop".to_string() } else { "inc2".to_string() }
        })
        .set_entry("inc")
        .build();

    let state = GraphState::new(DemoState { count: 1 });
    let out = graph.invoke(state).await.unwrap();
    assert_eq!(out.data.count, 3);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test graph_conditional -v`
Expected: FAIL with missing `add_conditional_edge`.

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/graph.rs
pub type Condition<S> = Box<dyn Fn(&GraphState<S>) -> String + Send + Sync>;

pub struct GraphBuilder<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    edges: HashMap<String, String>,
    conditional: HashMap<String, Condition<S>>,
    entry: Option<String>,
}

impl<S: StateSchema> GraphBuilder<S> {
    pub fn add_conditional_edge<F>(mut self, from: &str, condition: F) -> Self
    where
        F: Fn(&GraphState<S>) -> String + Send + Sync + 'static,
    {
        self.conditional.insert(from.to_string(), Box::new(condition));
        self
    }

    pub fn build(self) -> ExecutableGraph<S> {
        ExecutableGraph {
            nodes: self.nodes,
            edges: self.edges,
            conditional: self.conditional,
            entry: self.entry.expect("entry"),
        }
    }
}

pub struct ExecutableGraph<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    edges: HashMap<String, String>,
    conditional: HashMap<String, Condition<S>>,
    entry: String,
}

impl<S: StateSchema> ExecutableGraph<S> {
    pub async fn invoke(&self, mut state: GraphState<S>) -> Result<GraphState<S>, WesichainError> {
        let mut current = self.entry.clone();
        loop {
            let node = self.nodes.get(&current).expect("node");
            let update = node.invoke(state).await?;
            state = GraphState::new(update.data);

            if let Some(condition) = self.conditional.get(&current) {
                current = condition(&state);
                continue;
            }

            match self.edges.get(&current) {
                Some(next) => current = next.clone(),
                None => break,
            }
        }
        Ok(state)
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test graph_conditional -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/graph.rs wesichain-graph/tests/graph_conditional.rs
git commit -m "feat(graph): add conditional edges"
```

---

### Task 5: Add checkpointer trait + in-memory impl

**Files:**
- Create: `wesichain-graph/src/checkpoint.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Modify: `wesichain-graph/Cargo.toml`
- Test: `wesichain-graph/tests/checkpoint.rs`

**Step 1: Write the failing test**

```rust
use wesichain_graph::{Checkpoint, Checkpointer, GraphState, InMemoryCheckpointer, StateSchema};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState { count: i32 }

impl StateSchema for DemoState {}

#[tokio::test]
async fn checkpointer_roundtrip() {
    let checkpointer = InMemoryCheckpointer::default();
    let state = GraphState::new(DemoState { count: 1 });
    let checkpoint = Checkpoint::new("thread-1".to_string(), state);
    checkpointer.save(&checkpoint).await.unwrap();
    let loaded = checkpointer.load("thread-1").await.unwrap();
    assert_eq!(loaded.unwrap().state.data.count, 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test checkpoint -v`
Expected: FAIL with missing `Checkpointer`.

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/checkpoint.rs
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::{GraphState, StateSchema};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Checkpoint<S: StateSchema> {
    pub thread_id: String,
    pub state: GraphState<S>,
}

impl<S: StateSchema> Checkpoint<S> {
    pub fn new(thread_id: String, state: GraphState<S>) -> Self {
        Self { thread_id, state }
    }
}

#[async_trait::async_trait]
pub trait Checkpointer<S: StateSchema>: Send + Sync {
    async fn save(&self, checkpoint: &Checkpoint<S>) -> Result<(), crate::GraphError>;
    async fn load(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>, crate::GraphError>;
}

#[derive(Default, Clone)]
pub struct InMemoryCheckpointer<S: StateSchema> {
    inner: Arc<RwLock<HashMap<String, Checkpoint<S>>>>,
}

#[async_trait::async_trait]
impl<S: StateSchema> Checkpointer<S> for InMemoryCheckpointer<S> {
    async fn save(&self, checkpoint: &Checkpoint<S>) -> Result<(), crate::GraphError> {
        let mut guard = self.inner.write().map_err(|_| crate::GraphError::Checkpoint("lock".into()))?;
        guard.insert(checkpoint.thread_id.clone(), checkpoint.clone());
        Ok(())
    }

    async fn load(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>, crate::GraphError> {
        let guard = self.inner.read().map_err(|_| crate::GraphError::Checkpoint("lock".into()))?;
        Ok(guard.get(thread_id).cloned())
    }
}
```

```rust
// wesichain-graph/src/lib.rs
mod checkpoint;

pub use checkpoint::{Checkpoint, Checkpointer, InMemoryCheckpointer};
```

```toml
# wesichain-graph/Cargo.toml
[dependencies]
async-trait = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test checkpoint -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/checkpoint.rs wesichain-graph/src/lib.rs wesichain-graph/Cargo.toml wesichain-graph/tests/checkpoint.rs
git commit -m "feat(graph): add in-memory checkpointer"
```

---

### Task 6: Add checkpointing to graph execution

**Files:**
- Modify: `wesichain-graph/src/graph.rs`
- Test: `wesichain-graph/tests/graph_checkpoint.rs`

**Step 1: Write the failing test**

```rust
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, WesichainError, StreamEvent};
use wesichain_graph::{GraphBuilder, GraphState, InMemoryCheckpointer, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState { count: i32 }

impl StateSchema for DemoState {}

struct AddOne;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for AddOne {
    async fn invoke(&self, input: GraphState<DemoState>) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState { count: input.data.count + 1 }))
    }
    fn stream(&self, _input: GraphState<DemoState>) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
async fn graph_saves_checkpoint_each_step() {
    let checkpointer = InMemoryCheckpointer::default();
    let graph = GraphBuilder::new()
        .add_node("one", AddOne)
        .set_entry("one")
        .with_checkpointer(checkpointer.clone(), "thread-1")
        .build();

    let state = GraphState::new(DemoState { count: 1 });
    let out = graph.invoke(state).await.unwrap();
    assert_eq!(out.data.count, 2);

    let loaded = checkpointer.load("thread-1").await.unwrap().unwrap();
    assert_eq!(loaded.state.data.count, 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test graph_checkpoint -v`
Expected: FAIL with missing `with_checkpointer`.

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/graph.rs
use crate::{Checkpoint, Checkpointer};

pub struct GraphBuilder<S: StateSchema> {
    // existing fields...
    checkpointer: Option<(Box<dyn Checkpointer<S>>, String)>,
}

impl<S: StateSchema> GraphBuilder<S> {
    pub fn with_checkpointer<C>(mut self, checkpointer: C, thread_id: &str) -> Self
    where
        C: Checkpointer<S> + 'static,
    {
        self.checkpointer = Some((Box::new(checkpointer), thread_id.to_string()));
        self
    }
}

pub struct ExecutableGraph<S: StateSchema> {
    // existing fields...
    checkpointer: Option<(Box<dyn Checkpointer<S>>, String)>,
}

impl<S: StateSchema> ExecutableGraph<S> {
    pub async fn invoke(&self, mut state: GraphState<S>) -> Result<GraphState<S>, WesichainError> {
        let mut current = self.entry.clone();
        loop {
            let node = self.nodes.get(&current).expect("node");
            let update = node.invoke(state).await?;
            state = GraphState::new(update.data);

            if let Some((checkpointer, thread_id)) = &self.checkpointer {
                let checkpoint = Checkpoint::new(thread_id.clone(), state.clone());
                checkpointer.save(&checkpoint).await.map_err(|e| WesichainError::CheckpointFailed(e.to_string()))?;
            }

            // conditional / edge logic...
        }
        Ok(state)
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test graph_checkpoint -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/graph.rs wesichain-graph/tests/graph_checkpoint.rs
git commit -m "feat(graph): add checkpointing"
```

---

## Phase 3 Acceptance Criteria
- `wesichain-graph` crate exists and compiles.
- GraphState + StateUpdate support typed state merging (last-write-wins).
- GraphBuilder supports add_node, add_edge, add_conditional_edge, set_entry.
- ExecutableGraph runs nodes sequentially and supports conditionals.
- Checkpointer trait with in-memory implementation and graph-level checkpointing.

## Out of Scope (Phase 3)
- Parallel branches and graph-wide streaming.
- Advanced merge reducers.
- Persistent DB checkpointers (SQLite/Postgres).
