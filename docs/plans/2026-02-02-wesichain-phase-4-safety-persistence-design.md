# Wesichain Phase 4 Safety + Persistence Design

Date: 2026-02-02
Status: Draft

## Goals
- Add execution safety guards (max steps + cycle detection) with LangGraph-like per-invoke overrides.
- Add persistent checkpointing with append-only history in JSONL.
- Keep Phase 3 APIs intact; changes are additive and low overhead.

## Non-goals
- Reducers/merge strategies beyond last-write-wins.
- Streaming propagation across graph execution.
- Parallel fan-out, petgraph migration, or advanced checkpoint backends.

## Key Decisions
- Defaults: `max_steps = 50`, `cycle_detection = true`, `cycle_window = 20`.
- Per-invoke overrides with graph-level defaults (per-invoke wins).
- Cycle detection is simple repeat detection within the last N nodes.
- `Checkpoint` stays minimal; JSONL stores `CheckpointRecord { seq, created_at, checkpoint }`.
- File layout: one JSONL file per thread at `checkpoints/{sanitized_thread_id}.jsonl`.
- `CycleDetected` error includes `node` and recent window.

## Architecture and API

### Execution config and overrides
Introduce a graph-level default config with per-invoke overrides.

```rust
#[derive(Clone, Debug)]
pub struct ExecutionConfig {
    pub max_steps: Option<usize>,
    pub cycle_detection: bool,
    pub cycle_window: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ExecutionOptions {
    pub max_steps: Option<usize>,
    pub cycle_detection: Option<bool>,
    pub cycle_window: Option<usize>,
}
```

- `GraphBuilder::with_default_config(config)` sets defaults on the compiled graph.
- `ExecutableGraph::invoke(state)` uses defaults.
- `ExecutableGraph::invoke_with_options(state, options)` merges per-invoke overrides into defaults.

Merge semantics: per-invoke values take precedence when provided.

### Safety in execution loop
Per-invoke runtime state tracks `step_count` and a fixed-size deque of recent node IDs.

- Increment `step_count` before running each node.
- If `max_steps` is set and exceeded, return `GraphError::MaxStepsExceeded { max, reached }`.
- If cycle detection is enabled, push the current node into the deque (capacity `cycle_window`).
  If the current node appears twice in the window, return `GraphError::CycleDetected { node, recent }`.

The safety checks are O(1) per step and do not affect graph structure or node execution.

### File checkpointer (JSONL)
Add a `FileCheckpointer` implementing `Checkpointer` with append-only JSONL history.

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CheckpointRecord<S: StateSchema> {
    pub seq: u64,
    pub created_at: String,
    pub checkpoint: Checkpoint<S>,
}
```

- `seq` is monotonic per thread (start at 1).
- `created_at` is ISO8601 (UTC).
- Each `save()` appends one JSON line to `{sanitized_thread_id}.jsonl`.

Sanitization replaces `\/ : * ? " < > |` with `_`, trims leading/trailing dots and whitespace, and falls back to `thread-<short-hash>` if empty.

`load_latest(thread_id)` returns the last valid record's checkpoint. History access uses a separate helper or trait (e.g., `HistoryCheckpointer`) that streams the JSONL file and returns `CheckpointMetadata { seq, created_at }` without loading full state.

### Error handling
Add graph-specific errors for safety and IO:

- `GraphError::MaxStepsExceeded { max, reached }`
- `GraphError::CycleDetected { node, recent }`
- `GraphError::Checkpoint(String)` (IO/serde failures)

Graph errors propagate through `ExecutableGraph::invoke*` and map to `WesichainError::CheckpointFailed` when needed.

## Data Flow
1. Merge defaults + per-invoke overrides into effective config.
2. Execute node.
3. Apply update to state.
4. Save checkpoint (if configured).
5. Resolve next node via conditional or edge.
6. Repeat until no next node or guard triggers.

Resume path:
- `load_latest(thread_id)` restores the most recent `Checkpoint` state and proceeds from there.

## Testing Plan
Safety tests:
- max steps enforced.
- cycle detection triggers on oscillation.
- overrides take precedence over defaults.

Persistence tests:
- JSONL append writes multiple records.
- load latest returns most recent state.
- list history returns sorted metadata.
- corrupt last line returns a clear error (MVP choice).

Integration tests:
- graph with file checkpointer saves after each node.
- resume from checkpoint restores state.

## Examples
- Looping graph to show `MaxStepsExceeded` and `CycleDetected` behavior.
- Resume-after-crash example using JSONL checkpointer.
- README snippet showing per-invoke overrides.

## Rollout
- Add defaults and new APIs without breaking existing call sites.
- Document defaults and override usage.
- Release as the next minor version.

## Future Work
- Reducers/merge strategies.
- Streaming execution.
- Retention policies, compression, or bincode format.
- Richer checkpoint metadata (node, source, parent).
