# Wesichain Phase 7 Stateful Graphs Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement Phase 7 stateful graphs (reducers, checkpoints, interrupts, observability, streaming, agent loop) while keeping existing GraphBuilder APIs backward-compatible.

**Architecture:** Extend `wesichain-graph` with a merge hook on state, a petgraph-backed compiled program, and a sequential execution loop that supports interrupts, checkpoint metadata, and observer/streaming events. Keep current `GraphBuilder`/`ExecutableGraph` public APIs intact by adding additive methods and wrappers.

**Tech Stack:** Rust 1.75, tokio, async-trait, serde, thiserror, petgraph, tracing, chrono.

**Skill references:** @superpowers:test-driven-development, @superpowers:verification-before-completion, @superpowers:systematic-debugging

---

### Task 1: Add merge hook and reducer helpers

**Files:**
- Modify: `wesichain-graph/src/state.rs`
- Create: `wesichain-graph/src/reducer.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Test: `wesichain-graph/tests/state_reducer.rs`

**Step 1: Write the failing test**

```rust
use serde::{Deserialize, Serialize};
use wesichain_graph::{GraphState, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct MergeState {
    messages: Vec<String>,
    count: i32,
}

impl StateSchema for MergeState {
    fn merge(current: &Self, update: Self) -> Self {
        let mut merged = current.clone();
        merged.messages.extend(update.messages);
        merged.count += update.count;
        merged
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct OverrideState {
    count: i32,
}

impl StateSchema for OverrideState {}

#[test]
fn state_merge_appends_and_adds() {
    let base = GraphState::new(MergeState {
        messages: vec!["a".to_string()],
        count: 1,
    });
    let update = StateUpdate::new(MergeState {
        messages: vec!["b".to_string(), "c".to_string()],
        count: 2,
    });
    let merged = base.apply_update(update);
    assert_eq!(merged.data.messages, vec!["a", "b", "c"]);
    assert_eq!(merged.data.count, 3);
}

#[test]
fn state_merge_defaults_to_override() {
    let base = GraphState::new(OverrideState { count: 1 });
    let update = StateUpdate::new(OverrideState { count: 9 });
    let merged = base.apply_update(update);
    assert_eq!(merged.data.count, 9);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test state_reducer -v`
Expected: FAIL (missing `apply_update` and `StateSchema::merge`).

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/state.rs
pub trait StateSchema:
    Serialize + DeserializeOwned + Clone + Default + Send + Sync + 'static
{
    fn merge(current: &Self, update: Self) -> Self {
        update
    }
}

pub trait StateReducer: StateSchema {
    fn merge(current: &Self, update: Self) -> Self {
        <Self as StateSchema>::merge(current, update)
    }
}

impl<T: StateSchema> StateReducer for T {}

impl<S: StateSchema> GraphState<S> {
    pub fn apply_update(self, update: StateUpdate<S>) -> Self {
        Self {
            data: S::merge(&self.data, update.data),
        }
    }

    pub fn apply(self, update: StateUpdate<S>) -> Self {
        self.apply_update(update)
    }
}
```

```rust
// wesichain-graph/src/reducer.rs
use std::collections::HashMap;

pub struct AppendVec;
impl AppendVec {
    pub fn merge<T: Clone>(current: &Vec<T>, mut update: Vec<T>) -> Vec<T> {
        let mut out = current.clone();
        out.append(&mut update);
        out
    }
}

pub struct MergeMap;
impl MergeMap {
    pub fn merge<K: Eq + std::hash::Hash + Clone, V: Clone>(
        current: &HashMap<K, V>,
        update: HashMap<K, V>,
    ) -> HashMap<K, V> {
        let mut out = current.clone();
        out.extend(update);
        out
    }
}

pub struct AddCounter;
impl AddCounter {
    pub fn merge(current: &i64, update: i64) -> i64 {
        current + update
    }
}

pub struct Override;
impl Override {
    pub fn merge<T>(_current: &T, update: T) -> T {
        update
    }
}
```

```rust
// wesichain-graph/src/lib.rs
mod reducer;

pub use reducer::{AddCounter, AppendVec, MergeMap, Override};
pub use state::{GraphState, StateReducer, StateSchema, StateUpdate};
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test state_reducer --test state -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/state.rs wesichain-graph/src/reducer.rs wesichain-graph/src/lib.rs wesichain-graph/tests/state_reducer.rs
git commit -m "feat(graph): add state merge reducers"
```

---

### Task 2: Add START/END and compile to a petgraph program

**Files:**
- Create: `wesichain-graph/src/program.rs`
- Modify: `wesichain-graph/src/graph.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Modify: `wesichain-graph/Cargo.toml`
- Test: `wesichain-graph/tests/graph_program.rs`

**Step 1: Write the failing test**

```rust
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{GraphBuilder, GraphState, StateSchema, StateUpdate, START};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

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
async fn graph_respects_start_constant() {
    let graph = GraphBuilder::new()
        .add_node("inc", Inc)
        .add_edge(START, "inc")
        .set_entry("inc")
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    let out = graph.invoke(state).await.unwrap();
    assert_eq!(out.data.count, 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test graph_program -v`
Expected: FAIL (missing START constant / program support).

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/program.rs
use std::collections::HashMap;
use petgraph::graph::{Graph, NodeIndex};

use crate::{GraphState, StateSchema, StateUpdate};
use wesichain_core::Runnable;

pub struct NodeData<S: StateSchema> {
    pub name: String,
    pub runnable: Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Default,
}

pub struct GraphProgram<S: StateSchema> {
    pub graph: Graph<NodeData<S>, EdgeKind>,
    pub name_to_index: HashMap<String, NodeIndex>,
}
```

```rust
// wesichain-graph/src/lib.rs
mod program;

pub const START: &str = "__start";
pub const END: &str = "__end";

pub use program::{EdgeKind, GraphProgram, NodeData};
```

```rust
// wesichain-graph/src/graph.rs (partial, compile to program)
use petgraph::graph::Graph;
use crate::{EdgeKind, GraphProgram, NodeData, START};

pub fn build_program(self) -> GraphProgram<S> {
    let mut graph = Graph::new();
    let mut name_to_index = HashMap::new();

    for (name, runnable) in self.nodes {
        let index = graph.add_node(NodeData { name: name.clone(), runnable });
        name_to_index.insert(name, index);
    }

    for (from, to) in self.edges.iter() {
        if from == START {
            continue;
        }
        let from_idx = name_to_index[from];
        let to_idx = name_to_index[to];
        graph.add_edge(from_idx, to_idx, EdgeKind::Default);
    }

    GraphProgram { graph, name_to_index }
}
```

```toml
# wesichain-graph/Cargo.toml
petgraph = "0.6"
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test graph_program -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/program.rs wesichain-graph/src/graph.rs wesichain-graph/src/lib.rs wesichain-graph/Cargo.toml wesichain-graph/tests/graph_program.rs
git commit -m "feat(graph): add START/END and compiled program"
```

---

### Task 3: Expand GraphError and add GraphResult wrappers

**Files:**
- Modify: `wesichain-graph/src/error.rs`
- Modify: `wesichain-graph/src/graph.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Test: `wesichain-graph/tests/graph_errors.rs`

**Step 1: Write the failing test**

```rust
use serde::{Deserialize, Serialize};
use wesichain_graph::{GraphBuilder, GraphError, GraphState, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

#[tokio::test]
async fn graph_returns_missing_node_error() {
    let graph = GraphBuilder::new().set_entry("missing").build();
    let state = GraphState::new(DemoState { count: 0 });
    let err = graph.invoke_graph(state).await.unwrap_err();
    assert!(matches!(err, GraphError::MissingNode { .. }));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test graph_errors -v`
Expected: FAIL (no GraphError::MissingNode or invoke_graph).

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/error.rs
#[derive(Debug, Error)]
pub enum GraphError {
    #[error("checkpoint failed: {0}")]
    Checkpoint(String),
    #[error("node failed: {node}")]
    NodeFailed { node: String, source: String },
    #[error("missing node: {node}")]
    MissingNode { node: String },
    #[error("invalid edge to '{node}'")]
    InvalidEdge { node: String },
    #[error("Max steps exceeded: reached {reached}, limit {max}")]
    MaxStepsExceeded { max: usize, reached: usize },
    #[error("Cycle detected: node '{node}' repeated in recent window")]
    CycleDetected { node: String, recent: Vec<String> },
    #[error("interrupted")]
    Interrupted,
}
```

```rust
// wesichain-graph/src/graph.rs (partial)
pub async fn invoke_graph(
    &self,
    state: GraphState<S>,
) -> Result<GraphState<S>, GraphError> {
    // placeholder: call invoke_graph_with_options
    self.invoke_graph_with_options(state, ExecutionOptions::default()).await
}

pub async fn invoke_graph_with_options(
    &self,
    mut state: GraphState<S>,
    options: ExecutionOptions,
) -> Result<GraphState<S>, GraphError> {
    if !self.nodes.contains_key(&self.entry) {
        return Err(GraphError::MissingNode { node: self.entry.clone() });
    }
    // existing loop translated to GraphError
}

pub async fn invoke(&self, state: GraphState<S>) -> Result<GraphState<S>, WesichainError> {
    self.invoke_graph(state)
        .await
        .map_err(|err| WesichainError::Custom(err.to_string()))
}
```

```rust
// wesichain-graph/src/lib.rs
pub use error::GraphError;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test graph_errors -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/error.rs wesichain-graph/src/graph.rs wesichain-graph/src/lib.rs wesichain-graph/tests/graph_errors.rs
git commit -m "feat(graph): expand graph errors and invoke_graph"
```

---

### Task 4: Add interrupts and GraphInterrupt

**Files:**
- Modify: `wesichain-graph/src/graph.rs`
- Create: `wesichain-graph/src/interrupt.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Test: `wesichain-graph/tests/graph_interrupt.rs`

**Step 1: Write the failing test**

```rust
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{GraphBuilder, GraphInterrupt, GraphState, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

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
async fn graph_interrupts_before_node() {
    let graph = GraphBuilder::new()
        .add_node("inc", Inc)
        .set_entry("inc")
        .with_interrupt_before(["inc"]) 
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    let result = graph.invoke_graph(state).await;
    assert!(matches!(result, Err(wesichain_graph::GraphError::Interrupted)));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test graph_interrupt -v`
Expected: FAIL (missing interrupt methods and GraphInterrupt type).

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/interrupt.rs
use crate::GraphState;
use crate::StateSchema;

#[derive(Clone, Debug)]
pub struct GraphInterrupt<S: StateSchema> {
    pub node: String,
    pub state: GraphState<S>,
}
```

```rust
// wesichain-graph/src/graph.rs (fields)
interrupt_before: Vec<String>,
interrupt_after: Vec<String>,

pub fn with_interrupt_before<I, S2>(mut self, nodes: I) -> Self
where
    I: IntoIterator<Item = S2>,
    S2: Into<String>,
{
    self.interrupt_before = nodes.into_iter().map(Into::into).collect();
    self
}

pub fn with_interrupt_after<I, S2>(mut self, nodes: I) -> Self
where
    I: IntoIterator<Item = S2>,
    S2: Into<String>,
{
    self.interrupt_after = nodes.into_iter().map(Into::into).collect();
    self
}

// in invoke_graph loop
if self.interrupt_before.contains(&current) {
    return Err(GraphError::Interrupted);
}

// after checkpoint
if self.interrupt_after.contains(&current) {
    return Err(GraphError::Interrupted);
}
```

```rust
// wesichain-graph/src/lib.rs
mod interrupt;
pub use interrupt::GraphInterrupt;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test graph_interrupt -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/interrupt.rs wesichain-graph/src/graph.rs wesichain-graph/src/lib.rs wesichain-graph/tests/graph_interrupt.rs
git commit -m "feat(graph): add interrupt hooks"
```

---

### Task 5: Expand checkpoint metadata and history

**Files:**
- Modify: `wesichain-graph/src/checkpoint.rs`
- Modify: `wesichain-graph/src/file_checkpointer.rs`
- Modify: `wesichain-graph/src/graph.rs`
- Test: `wesichain-graph/tests/checkpoint.rs`
- Test: `wesichain-graph/tests/checkpointer_file.rs`

**Step 1: Write the failing test**

```rust
use serde::{Deserialize, Serialize};
use wesichain_graph::{Checkpoint, GraphState, InMemoryCheckpointer, StateSchema};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

#[tokio::test]
async fn checkpointer_records_step_and_node() {
    let checkpointer = InMemoryCheckpointer::default();
    let state = GraphState::new(DemoState { count: 1 });
    let checkpoint = Checkpoint::new("thread-1".to_string(), state, 3, "inc".to_string());
    checkpointer.save(&checkpoint).await.unwrap();
    let loaded = checkpointer.load("thread-1").await.unwrap().unwrap();
    assert_eq!(loaded.step, 3);
    assert_eq!(loaded.node, "inc");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test checkpoint -v`
Expected: FAIL (missing step/node fields).

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/checkpoint.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Checkpoint<S: StateSchema> {
    pub thread_id: String,
    pub state: GraphState<S>,
    pub step: u64,
    pub node: String,
    pub created_at: String,
}

impl<S: StateSchema> Checkpoint<S> {
    pub fn new(thread_id: String, state: GraphState<S>, step: u64, node: String) -> Self {
        Self {
            thread_id,
            state,
            step,
            node,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Default, Clone)]
pub struct InMemoryCheckpointer<S: StateSchema> {
    inner: Arc<RwLock<HashMap<String, Vec<Checkpoint<S>>>>>,
}

// save pushes into history, load returns last()
```

```rust
// wesichain-graph/src/graph.rs (checkpoint usage)
let checkpoint = Checkpoint::new(thread_id.clone(), state.clone(), step_count as u64, current.clone());
```

```rust
// wesichain-graph/src/file_checkpointer.rs
let record = CheckpointRecord {
    seq,
    created_at: checkpoint.created_at.clone(),
    checkpoint: checkpoint.clone(),
};
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test checkpoint --test checkpointer_file -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/checkpoint.rs wesichain-graph/src/file_checkpointer.rs wesichain-graph/src/graph.rs wesichain-graph/tests/checkpoint.rs wesichain-graph/tests/checkpointer_file.rs
git commit -m "feat(graph): add checkpoint metadata"
```

---

### Task 6: Add observer hooks and tracing

**Files:**
- Create: `wesichain-graph/src/observer.rs`
- Modify: `wesichain-graph/src/graph.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Test: `wesichain-graph/tests/graph_observer.rs`

**Step 1: Write the failing test**

```rust
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{GraphBuilder, GraphState, Observer, StateSchema, StateUpdate};

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

#[derive(Default)]
struct CollectingObserver {
    events: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl Observer for CollectingObserver {
    async fn on_node_start(&self, node_id: &str, _input: &serde_json::Value) {
        self.events.lock().unwrap().push(format!("start:{node_id}"));
    }

    async fn on_node_end(&self, node_id: &str, _output: &serde_json::Value, _duration_ms: u128) {
        self.events.lock().unwrap().push(format!("end:{node_id}"));
    }

    async fn on_error(&self, _node_id: &str, _error: &GraphError) {}
}

#[tokio::test]
async fn observer_receives_node_events() {
    let observer = CollectingObserver::default();
    let events = observer.events.clone();
    let graph = GraphBuilder::new()
        .add_node("inc", Inc)
        .set_entry("inc")
        .with_observer(Arc::new(observer))
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    graph.invoke_graph(state).await.unwrap();
    assert_eq!(events.lock().unwrap().as_slice(), ["start:inc", "end:inc"]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test graph_observer -v`
Expected: FAIL (no Observer trait or hook).

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/observer.rs
#[async_trait::async_trait]
pub trait Observer: Send + Sync + 'static {
    async fn on_node_start(&self, node_id: &str, input: &serde_json::Value);
    async fn on_node_end(&self, node_id: &str, output: &serde_json::Value, duration_ms: u128);
    async fn on_error(&self, node_id: &str, error: &GraphError);
    async fn on_tool_call(&self, _node_id: &str, _tool_name: &str, _args: &serde_json::Value) {}
    async fn on_tool_result(&self, _node_id: &str, _tool_name: &str, _result: &serde_json::Value) {}
    async fn on_checkpoint_saved(&self, _node_id: &str) {}
}
```

```rust
// wesichain-graph/src/graph.rs (field + builder)
observer: Option<Arc<dyn Observer>>,

pub fn with_observer(mut self, observer: Arc<dyn Observer>) -> Self {
    self.observer = Some(observer);
    self
}

// in invoke_graph loop
if let Some(observer) = &self.observer {
    let input_value = serde_json::to_value(&state.data)?;
    observer.on_node_start(&current, &input_value).await;
}
// after node
if let Some(observer) = &self.observer {
    let output_value = serde_json::to_value(&state.data)?;
    observer.on_node_end(&current, &output_value, duration_ms).await;
}
```

```rust
// wesichain-graph/src/lib.rs
mod observer;
pub use observer::Observer;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test graph_observer -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/observer.rs wesichain-graph/src/graph.rs wesichain-graph/src/lib.rs wesichain-graph/tests/graph_observer.rs
git commit -m "feat(graph): add observer hooks"
```

---

### Task 7: Add streaming GraphEvent API

**Files:**
- Create: `wesichain-graph/src/stream.rs`
- Modify: `wesichain-graph/src/graph.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Test: `wesichain-graph/tests/graph_stream.rs`

**Step 1: Write the failing test**

```rust
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{GraphBuilder, GraphEvent, GraphState, StateSchema, StateUpdate};

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
async fn stream_emits_node_events() {
    let graph = GraphBuilder::new()
        .add_node("inc", Inc)
        .set_entry("inc")
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    let mut events = graph.stream_invoke(state);
    let first = events.next().await.unwrap().unwrap();
    assert!(matches!(first, GraphEvent::NodeEnter { node } if node == "inc"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test graph_stream -v`
Expected: FAIL (no GraphEvent or stream_invoke).

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/stream.rs
use crate::GraphError;
use wesichain_core::StreamEvent;

#[derive(Debug)]
pub enum GraphEvent {
    NodeEnter { node: String },
    NodeExit { node: String },
    CheckpointSaved { node: String },
    StreamEvent(StreamEvent),
    Error(GraphError),
}
```

```rust
// wesichain-graph/src/graph.rs (stream_invoke)
pub fn stream_invoke(
    &self,
    state: GraphState<S>,
) -> futures::stream::BoxStream<'_, Result<GraphEvent, GraphError>> {
    // wrap invoke_graph loop to emit GraphEvent::NodeEnter/NodeExit
    // minimal: emit enter + exit + return final state
}
```

```rust
// wesichain-graph/src/lib.rs
mod stream;
pub use stream::GraphEvent;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test graph_stream -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/stream.rs wesichain-graph/src/graph.rs wesichain-graph/src/lib.rs wesichain-graph/tests/graph_stream.rs
git commit -m "feat(graph): add streaming graph events"
```

---

### Task 8: Add ToolNode for ReAct loop integration

**Files:**
- Create: `wesichain-graph/src/tool_node.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Test: `wesichain-graph/tests/tool_node.rs`

**Step 1: Write the failing test**

```rust
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use wesichain_graph::{GraphState, StateSchema, StateUpdate, ToolNode};
use wesichain_llm::{Message, Role, ToolCall};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct AgentState {
    tool_calls: Vec<ToolCall>,
    tool_results: Vec<Message>,
}

impl StateSchema for AgentState {}

#[tokio::test]
async fn tool_node_executes_calls_and_appends_results() {
    let calls = vec![ToolCall { id: "1".into(), name: "echo".into(), args: serde_json::json!({"text": "hi"}) }];
    let state = GraphState::new(AgentState { tool_calls: calls, tool_results: Vec::new() });
    let node = ToolNode::new(vec![Arc::new(MockTool::default())]);
    let update = node.invoke(state).await.unwrap();
    assert_eq!(update.data.tool_results.len(), 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test tool_node -v`
Expected: FAIL (no ToolNode).

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/tool_node.rs
use std::sync::Arc;
use wesichain_core::{Runnable, WesichainError};
use wesichain_llm::{Message, Role, ToolCall};
use crate::{GraphState, StateSchema, StateUpdate};

pub trait HasToolCalls {
    fn tool_calls(&self) -> &Vec<ToolCall>;
    fn push_tool_result(&mut self, message: Message);
}

pub struct ToolNode {
    tools: Vec<Arc<dyn wesichain_agent::Tool>>,
}

impl ToolNode {
    pub fn new(tools: Vec<Arc<dyn wesichain_agent::Tool>>) -> Self {
        Self { tools }
    }
}

#[async_trait::async_trait]
impl<S> Runnable<GraphState<S>, StateUpdate<S>> for ToolNode
where
    S: StateSchema + HasToolCalls,
{
    async fn invoke(&self, input: GraphState<S>) -> Result<StateUpdate<S>, WesichainError> {
        let mut next = input.data.clone();
        for call in input.data.tool_calls() {
            let tool = self.tools.iter().find(|tool| tool.name() == call.name).ok_or_else(|| WesichainError::Custom("tool missing".into()))?;
            let output = tool.invoke(call.args.clone()).await?;
            next.push_tool_result(Message { role: Role::Tool, content: output.to_string(), tool_call_id: Some(call.id.clone()) });
        }
        Ok(StateUpdate::new(next))
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test tool_node -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/tool_node.rs wesichain-graph/src/lib.rs wesichain-graph/tests/tool_node.rs
git commit -m "feat(graph): add tool execution node"
```

---

### Task 9: Documentation + examples

**Files:**
- Create: `wesichain-graph/examples/react_agent.rs`
- Create: `wesichain-graph/examples/persistent_conversation.rs`
- Create: `wesichain-graph/examples/human_in_loop_review.rs`
- Create: `docs/migration/graph-workflows-to-wesichain.md`
- Modify: `README.md`

**Step 1: Write the documentation/examples**

Add minimal, runnable examples using mock tools (no API keys). Ensure each example includes a short comment with the run command.

**Step 2: Run examples to verify they compile**

Run: `cargo run -p wesichain-graph --example react_agent`
Expected: PASS (no panic)

**Step 3: Commit**

```bash
git add wesichain-graph/examples docs/migration/graph-workflows-to-wesichain.md README.md
git commit -m "docs(graph): add phase 7 examples and migration guide"
```

---

### Task 10: Benchmarks

**Files:**
- Create: `wesichain-graph/benches/graph_loop.rs`
- Modify: `wesichain-graph/Cargo.toml`
- Modify: `docs/plans/2026-02-04-wesichain-phase-7-stateful-graphs-design.md`

**Step 1: Write the benchmark**

Use Criterion to compare a 10-step loop with checkpointing enabled vs disabled. Emit per-step time and RSS delta.

**Step 2: Run benchmark to verify it executes**

Run: `cargo bench -p wesichain-graph --bench graph_loop`
Expected: PASS

**Step 3: Commit**

```bash
git add wesichain-graph/benches/graph_loop.rs wesichain-graph/Cargo.toml docs/plans/2026-02-04-wesichain-phase-7-stateful-graphs-design.md
git commit -m "bench(graph): add phase 7 loop benchmark"
```

---

## Notes
- Use `invoke_graph` for GraphError-first paths; keep existing `invoke` wrappers to avoid breaking API.
- Keep `GraphState::apply` as an alias for `apply_update` to preserve existing call sites.
- If tool types move out of `wesichain-agent`, update examples and tests accordingly.

---

Plan complete and saved to `docs/plans/2026-02-04-wesichain-phase-7-stateful-graphs-implementation-plan.md`. Two execution options:

1. Subagent-Driven (this session) - I dispatch fresh subagent per task, review between tasks, fast iteration
2. Parallel Session (separate) - Open new session with executing-plans, batch execution with checkpoints

Which approach?
