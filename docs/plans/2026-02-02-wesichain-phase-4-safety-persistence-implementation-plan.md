# Phase 4 Safety + Persistence Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add execution safety (max-steps + cycle detection) with per-invoke overrides and a JSONL file checkpointer with append-only history.

**Architecture:** Introduce `ExecutionConfig` defaults on `ExecutableGraph` with per-invoke `ExecutionOptions` overrides, then enforce guards in the execution loop. Add `FileCheckpointer` that writes `CheckpointRecord` lines to a per-thread JSONL file and exposes history metadata.

**Tech Stack:** Rust 1.75+, tokio, async-trait, serde, serde_json, thiserror, chrono, tempfile (dev).

---

### Task 1: ExecutionConfig + ExecutionOptions types

**Files:**
- Create: `wesichain-graph/src/config.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Test: `wesichain-graph/tests/execution_config.rs`

**Step 1: Write the failing test**

```rust
use wesichain_graph::{ExecutionConfig, ExecutionOptions};

#[test]
fn execution_config_defaults_and_merge() {
    let defaults = ExecutionConfig::default();
    assert_eq!(defaults.max_steps, Some(50));
    assert!(defaults.cycle_detection);
    assert_eq!(defaults.cycle_window, 20);

    let overrides = ExecutionOptions {
        max_steps: Some(5),
        cycle_detection: Some(false),
        cycle_window: Some(3),
    };
    let merged = defaults.merge(&overrides);
    assert_eq!(merged.max_steps, Some(5));
    assert!(!merged.cycle_detection);
    assert_eq!(merged.cycle_window, 3);

    let merged_empty = defaults.merge(&ExecutionOptions::default());
    assert_eq!(merged_empty.max_steps, Some(50));
    assert!(merged_empty.cycle_detection);
    assert_eq!(merged_empty.cycle_window, 20);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test execution_config -v`
Expected: FAIL with missing `ExecutionConfig`/`ExecutionOptions`.

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/config.rs
#[derive(Clone, Debug)]
pub struct ExecutionConfig {
    pub max_steps: Option<usize>,
    pub cycle_detection: bool,
    pub cycle_window: usize,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_steps: Some(50),
            cycle_detection: true,
            cycle_window: 20,
        }
    }
}

impl ExecutionConfig {
    pub fn merge(&self, overrides: &ExecutionOptions) -> Self {
        Self {
            max_steps: overrides.max_steps.or(self.max_steps),
            cycle_detection: overrides.cycle_detection.unwrap_or(self.cycle_detection),
            cycle_window: overrides.cycle_window.unwrap_or(self.cycle_window),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExecutionOptions {
    pub max_steps: Option<usize>,
    pub cycle_detection: Option<bool>,
    pub cycle_window: Option<usize>,
}
```

```rust
// wesichain-graph/src/lib.rs
mod config;

pub use config::{ExecutionConfig, ExecutionOptions};
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test execution_config -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/config.rs wesichain-graph/src/lib.rs wesichain-graph/tests/execution_config.rs
git commit -m "feat(graph): add execution config defaults"
```

---

### Task 2: Safety guards (max-steps + cycle detection)

**Files:**
- Modify: `wesichain-graph/src/error.rs`
- Modify: `wesichain-graph/src/graph.rs`
- Test: `wesichain-graph/tests/graph_safety.rs`

**Step 1: Write the failing tests**

```rust
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{
    ExecutionConfig, ExecutionOptions, GraphBuilder, GraphState, StateSchema, StateUpdate,
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

struct Inc;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for Inc {
    async fn invoke(
        &self,
        input: GraphState<DemoState>,
    ) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState {
            count: input.data.count + 1,
        }))
    }

    fn stream(
        &self,
        _input: GraphState<DemoState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
async fn graph_enforces_max_steps() {
    let graph = GraphBuilder::new()
        .add_node("inc", Inc)
        .add_edge("inc", "inc")
        .set_entry("inc")
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    let options = ExecutionOptions {
        max_steps: Some(2),
        cycle_detection: Some(false),
        cycle_window: None,
    };
    let err = graph.invoke_with_options(state, options).await.unwrap_err();
    assert!(err.to_string().contains("Max steps exceeded"));
}

#[tokio::test]
async fn graph_detects_cycle_in_recent_window() {
    let graph = GraphBuilder::new()
        .add_node("inc", Inc)
        .add_edge("inc", "inc")
        .set_entry("inc")
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    let options = ExecutionOptions {
        max_steps: Some(10),
        cycle_detection: Some(true),
        cycle_window: Some(2),
    };
    let err = graph.invoke_with_options(state, options).await.unwrap_err();
    assert!(err.to_string().contains("Cycle detected"));
}

#[tokio::test]
async fn graph_options_override_defaults() {
    let graph = GraphBuilder::new()
        .with_default_config(ExecutionConfig {
            max_steps: Some(1),
            cycle_detection: true,
            cycle_window: 2,
        })
        .add_node("one", Inc)
        .add_node("two", Inc)
        .add_edge("one", "two")
        .set_entry("one")
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    let options = ExecutionOptions {
        max_steps: Some(5),
        cycle_detection: Some(false),
        cycle_window: None,
    };
    let out = graph.invoke_with_options(state, options).await.unwrap();
    assert_eq!(out.data.count, 2);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-graph --test graph_safety -v`
Expected: FAIL with missing `invoke_with_options`/config.

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/error.rs
#[derive(Debug, Error)]
pub enum GraphError {
    #[error("checkpoint failed: {0}")]
    Checkpoint(String),
    #[error("Max steps exceeded: reached {reached}, limit {max}")]
    MaxStepsExceeded { max: usize, reached: usize },
    #[error("Cycle detected: node '{node}' repeated in recent window")]
    CycleDetected { node: String, recent: Vec<String> },
}
```

```rust
// wesichain-graph/src/graph.rs (key additions)
use std::collections::{HashMap, VecDeque};

use crate::{ExecutionConfig, ExecutionOptions};

pub struct GraphBuilder<S: StateSchema> {
    // ...
    default_config: ExecutionConfig,
}

impl<S: StateSchema> GraphBuilder<S> {
    pub fn new() -> Self {
        Self {
            // ...
            default_config: ExecutionConfig::default(),
        }
    }

    pub fn with_default_config(mut self, config: ExecutionConfig) -> Self {
        self.default_config = config;
        self
    }

    pub fn build(self) -> ExecutableGraph<S> {
        ExecutableGraph {
            // ...
            default_config: self.default_config,
        }
    }
}

pub struct ExecutableGraph<S: StateSchema> {
    // ...
    default_config: ExecutionConfig,
}

impl<S: StateSchema> ExecutableGraph<S> {
    pub async fn invoke(&self, state: GraphState<S>) -> Result<GraphState<S>, WesichainError> {
        self.invoke_with_options(state, ExecutionOptions::default())
            .await
    }

    pub async fn invoke_with_options(
        &self,
        mut state: GraphState<S>,
        options: ExecutionOptions,
    ) -> Result<GraphState<S>, WesichainError> {
        let effective = self.default_config.merge(&options);
        let mut step_count = 0usize;
        let mut recent: VecDeque<String> = VecDeque::new();
        let mut current = self.entry.clone();

        loop {
            if let Some(max) = effective.max_steps {
                if step_count >= max {
                    return Err(WesichainError::Custom(
                        GraphError::MaxStepsExceeded { max, reached: step_count }.to_string(),
                    ));
                }
            }
            step_count += 1;

            if effective.cycle_detection {
                if recent.len() == effective.cycle_window {
                    recent.pop_front();
                }
                recent.push_back(current.clone());
                let count = recent.iter().filter(|node| **node == current).count();
                if count >= 2 {
                    return Err(WesichainError::Custom(
                        GraphError::CycleDetected {
                            node: current.clone(),
                            recent: recent.iter().cloned().collect(),
                        }
                        .to_string(),
                    ));
                }
            }

            let node = self.nodes.get(&current).expect("node");
            let update = node.invoke(state).await?;
            state = GraphState::new(update.data);
            // existing checkpointer logic
            // existing conditional/edge logic
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-graph --test graph_safety -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/error.rs wesichain-graph/src/graph.rs wesichain-graph/tests/graph_safety.rs
git commit -m "feat(graph): add safety guards"
```

---

### Task 3: FileCheckpointer (JSONL history)

**Files:**
- Create: `wesichain-graph/src/file_checkpointer.rs`
- Modify: `wesichain-graph/src/checkpoint.rs`
- Modify: `wesichain-graph/src/lib.rs`
- Modify: `wesichain-graph/Cargo.toml`
- Test: `wesichain-graph/tests/checkpointer_file.rs`

**Step 1: Write the failing tests**

```rust
use serde::{Deserialize, Serialize};
use tempfile::tempdir;
use wesichain_graph::{
    Checkpoint, Checkpointer, FileCheckpointer, GraphState, HistoryCheckpointer, StateSchema,
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

#[tokio::test]
async fn file_checkpointer_appends_and_loads_latest() {
    let dir = tempdir().unwrap();
    let checkpointer = FileCheckpointer::new(dir.path());

    let first = Checkpoint::new("thread/1".to_string(), GraphState::new(DemoState { count: 1 }));
    let second = Checkpoint::new("thread/1".to_string(), GraphState::new(DemoState { count: 2 }));

    checkpointer.save(&first).await.unwrap();
    checkpointer.save(&second).await.unwrap();

    let loaded = checkpointer.load("thread/1").await.unwrap().unwrap();
    assert_eq!(loaded.state.data.count, 2);

    let path = dir.path().join("thread_1.jsonl");
    assert!(path.exists());
}

#[tokio::test]
async fn file_checkpointer_lists_metadata() {
    let dir = tempdir().unwrap();
    let checkpointer = FileCheckpointer::new(dir.path());

    let first = Checkpoint::new("thread-2".to_string(), GraphState::new(DemoState { count: 1 }));
    let second = Checkpoint::new("thread-2".to_string(), GraphState::new(DemoState { count: 2 }));

    checkpointer.save(&first).await.unwrap();
    checkpointer.save(&second).await.unwrap();

    let history = checkpointer.list_checkpoints("thread-2").await.unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].seq, 1);
    assert_eq!(history[1].seq, 2);
    assert!(!history[0].created_at.is_empty());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-graph --test checkpointer_file -v`
Expected: FAIL with missing file checkpointer types.

**Step 3: Write minimal implementation**

```rust
// wesichain-graph/src/checkpoint.rs (add metadata + history trait)
#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointMetadata {
    pub seq: u64,
    pub created_at: String,
}

#[async_trait::async_trait]
pub trait HistoryCheckpointer<S: StateSchema>: Send + Sync {
    async fn list_checkpoints(&self, thread_id: &str) -> Result<Vec<CheckpointMetadata>, GraphError>;
}
```

```rust
// wesichain-graph/src/file_checkpointer.rs
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    Checkpoint, CheckpointMetadata, Checkpointer, GraphError, HistoryCheckpointer, StateSchema,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "S: StateSchema")]
pub struct CheckpointRecord<S: StateSchema> {
    pub seq: u64,
    pub created_at: String,
    pub checkpoint: Checkpoint<S>,
}

#[derive(Clone, Debug)]
pub struct FileCheckpointer {
    base_dir: PathBuf,
}

impl FileCheckpointer {
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    fn sanitize_thread_id(thread_id: &str) -> String {
        let mut out = String::with_capacity(thread_id.len());
        for ch in thread_id.chars() {
            match ch {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => out.push('_'),
                c if c.is_control() => {}
                c => out.push(c),
            }
        }
        let trimmed = out.trim_matches(|c: char| c == '.' || c.is_whitespace() || c == '_');
        if trimmed.is_empty() {
            let mut hasher = DefaultHasher::new();
            thread_id.hash(&mut hasher);
            format!("thread-{:08x}", hasher.finish())
        } else {
            trimmed.to_string()
        }
    }

    fn thread_path(&self, thread_id: &str) -> PathBuf {
        let filename = format!("{}.jsonl", Self::sanitize_thread_id(thread_id));
        self.base_dir.join(filename)
    }

    fn next_seq<S: StateSchema>(&self, thread_id: &str) -> Result<u64, GraphError> {
        let path = self.thread_path(thread_id);
        if !path.exists() {
            return Ok(1);
        }
        let file = File::open(&path).map_err(|err| GraphError::Checkpoint(err.to_string()))?;
        let reader = BufReader::new(file);
        let mut last: Option<CheckpointRecord<S>> = None;
        for line in reader.lines() {
            let line = line.map_err(|err| GraphError::Checkpoint(err.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            last = Some(
                serde_json::from_str(&line).map_err(|err| GraphError::Checkpoint(err.to_string()))?,
            );
        }
        Ok(last.map(|record| record.seq + 1).unwrap_or(1))
    }
}

#[async_trait::async_trait]
impl<S: StateSchema> Checkpointer<S> for FileCheckpointer {
    async fn save(&self, checkpoint: &Checkpoint<S>) -> Result<(), GraphError> {
        fs::create_dir_all(&self.base_dir)
            .map_err(|err| GraphError::Checkpoint(err.to_string()))?;

        let path = self.thread_path(&checkpoint.thread_id);
        let seq = self.next_seq::<S>(&checkpoint.thread_id)?;
        let record = CheckpointRecord {
            seq,
            created_at: Utc::now().to_rfc3339(),
            checkpoint: checkpoint.clone(),
        };
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|err| GraphError::Checkpoint(err.to_string()))?;
        let line = serde_json::to_string(&record).map_err(|err| GraphError::Checkpoint(err.to_string()))?;
        file.write_all(format!("{line}\n").as_bytes())
            .map_err(|err| GraphError::Checkpoint(err.to_string()))?;
        Ok(())
    }

    async fn load(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>, GraphError> {
        let path = self.thread_path(thread_id);
        if !path.exists() {
            return Ok(None);
        }
        let file = File::open(&path).map_err(|err| GraphError::Checkpoint(err.to_string()))?;
        let reader = BufReader::new(file);
        let mut last: Option<CheckpointRecord<S>> = None;
        for line in reader.lines() {
            let line = line.map_err(|err| GraphError::Checkpoint(err.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            last = Some(
                serde_json::from_str(&line).map_err(|err| GraphError::Checkpoint(err.to_string()))?,
            );
        }
        Ok(last.map(|record| record.checkpoint))
    }
}

#[async_trait::async_trait]
impl<S: StateSchema> HistoryCheckpointer<S> for FileCheckpointer {
    async fn list_checkpoints(&self, thread_id: &str) -> Result<Vec<CheckpointMetadata>, GraphError> {
        let path = self.thread_path(thread_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = File::open(&path).map_err(|err| GraphError::Checkpoint(err.to_string()))?;
        let reader = BufReader::new(file);
        let mut history = Vec::new();
        for line in reader.lines() {
            let line = line.map_err(|err| GraphError::Checkpoint(err.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let record: CheckpointRecord<S> =
                serde_json::from_str(&line).map_err(|err| GraphError::Checkpoint(err.to_string()))?;
            history.push(CheckpointMetadata {
                seq: record.seq,
                created_at: record.created_at,
            });
        }
        Ok(history)
    }
}
```

```rust
// wesichain-graph/src/lib.rs
mod file_checkpointer;

pub use file_checkpointer::{CheckpointRecord, FileCheckpointer};
pub use checkpoint::{CheckpointMetadata, HistoryCheckpointer};
```

```toml
# wesichain-graph/Cargo.toml
[dependencies]
chrono = { version = "0.4", features = ["clock"] }

[dev-dependencies]
tempfile = "3"
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-graph --test checkpointer_file -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-graph/src/checkpoint.rs wesichain-graph/src/file_checkpointer.rs wesichain-graph/src/lib.rs wesichain-graph/Cargo.toml wesichain-graph/tests/checkpointer_file.rs
git commit -m "feat(graph): add JSONL file checkpointer"
```

---

### Task 4: Integration test for file checkpointer in graph execution

**Files:**
- Test: `wesichain-graph/tests/graph_file_checkpointer.rs`

**Step 1: Write the failing test**

```rust
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use tempfile::tempdir;
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{FileCheckpointer, GraphBuilder, GraphState, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

struct Inc;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for Inc {
    async fn invoke(
        &self,
        input: GraphState<DemoState>,
    ) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState {
            count: input.data.count + 1,
        }))
    }

    fn stream(
        &self,
        _input: GraphState<DemoState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
async fn graph_writes_checkpoint_history_to_file() {
    let dir = tempdir().unwrap();
    let checkpointer = FileCheckpointer::new(dir.path());
    let graph = GraphBuilder::new()
        .add_node("one", Inc)
        .add_node("two", Inc)
        .add_edge("one", "two")
        .set_entry("one")
        .with_checkpointer(checkpointer, "thread-1")
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    let out = graph.invoke(state).await.unwrap();
    assert_eq!(out.data.count, 2);

    let path = dir.path().join("thread-1.jsonl");
    let contents = std::fs::read_to_string(path).unwrap();
    assert_eq!(contents.lines().count(), 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-graph --test graph_file_checkpointer -v`
Expected: FAIL with missing file checkpointer exports or behavior.

**Step 3: Run test to verify it passes**

Run: `cargo test -p wesichain-graph --test graph_file_checkpointer -v`
Expected: PASS

**Step 4: Commit**

```bash
git add wesichain-graph/tests/graph_file_checkpointer.rs
git commit -m "test(graph): cover file checkpointer integration"
```

---

### Task 5: README snippet for safety + persistence

**Files:**
- Modify: `README.md`

**Step 1: Update README**

Add a short snippet showing `ExecutionOptions` overrides and `FileCheckpointer` usage.

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: document graph safety and file checkpointing"
```
