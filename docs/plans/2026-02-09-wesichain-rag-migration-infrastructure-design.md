# Wesichain Migration Infrastructure Design

Date: 2026-02-09  
Status: Locked and validated  
Scope: P0 migration infrastructure for Python-style agentic RAG workloads

## Goal

Deliver a migration-ready Wesichain path for projects like `sample-projects/simple-agentic-rag` by shipping a focused 80/20 core, transport adapters, and an opinionated but thin RAG facade that teams can adopt endpoint-by-endpoint.

## Locked Direction

- Build migration infrastructure, not a one-off app clone.
- Keep core primitives transport-neutral and backend-neutral.
- Prioritize focused 80 percent coverage now with explicit extension seams.
- Ship one strong compatibility adapter first (sample-compatible SSE), then generalize.
- Deliver value in vertical slices with runnable client artifacts every wave.

## 1) Architecture and Boundaries

Wesichain uses a two-layer migration architecture: stable core primitives plus thin adapters. The core remains Rust-native and wire-agnostic. `wesichain-core` owns the canonical event model used by execution runtimes. `wesichain-agent` and `wesichain-graph` emit ordered semantic events as part of normal execution. Persistence stays trait-driven through `Checkpointer<S>`, with SQLite and Postgres backends already available and file/in-memory behavior unchanged.

Adapters are a separate concern. They map semantic events to delivery transports and wire contracts without leaking transport decisions into core runtime logic. First target is SSE compatibility for the existing sample app format. Future adapters can target WebSocket, gRPC, Kafka, CLI TUI, or structured logs by consuming the same semantic stream.

To provide immediate migration value, add `wesichain-rag` as an opinionated facade crate that composes existing crates and does orchestration only. It should not re-implement loaders, splitters, checkpointers, or vector stores. This gives teams a single entry point that feels similar to existing Python `rag_engine` workflows while preserving full composability for advanced users.

## 2) Public APIs and Event Contract

### AgentEvent in `wesichain-core`

Add a focused event contract for migration parity and runtime introspection:

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AgentEvent {
    Status {
        stage: String,
        message: String,
        step: usize,
        thread_id: String,
    },
    Thought {
        content: String,
        step: usize,
        metadata: Option<serde_json::Value>,
    },
    ToolCall {
        id: String,
        tool_name: String,
        input: serde_json::Value,
        step: usize,
    },
    Observation {
        id: String,
        tool_name: String,
        output: serde_json::Value,
        step: usize,
    },
    Final {
        content: String,
        step: usize,
    },
    Error {
        message: String,
        step: usize,
        recoverable: bool,
        source: Option<String>,
    },
    Metadata {
        key: String,
        value: serde_json::Value,
    },
}
```

This keeps v1 lean and covers the primary migration path without variant explosion. Extension pressure is handled first through `metadata` and `Metadata`; additional custom variants can be added after real migration evidence.

### `wesichain-rag` facade surface

Expose a migration-friendly, thin facade:

- Builder: `with_llm`, `with_embedder`, `with_vector_store`, `with_checkpointer`, `with_splitter`, `with_loader_registry`, `with_event_buffer_size`, `with_max_retries`.
- Operations: `process_file`, `add_documents`, `similarity_search`, `similarity_search_with_score`, `query`, `query_stream`.
- Streaming: `query_stream(...) -> Stream<Item = Result<AgentEvent, RagError>>`.
- Session behavior: `thread_id` optional input; auto-generate when absent; return resolved `thread_id` in responses/events.

### SSE adapter (first compatibility target)

Ship `wesichain-rag::adapters::sse::SimpleAgenticRagV1` mapping:

- `Status` -> `event: status`
- `Thought`/`ToolCall`/`Observation` -> `event: trace`
- `Final` -> `event: answer`
- `Error` -> `event: error`
- heartbeat -> `event: ping`
- terminal marker -> `event: done`

## 3) Data Flow, Streaming Pipeline, and Error Handling

### Ingestion pipeline

`process_file` runs `loader -> splitter -> embedder -> vector_store.upsert` and emits coarse progress (`loading`, `splitting`, `indexing`, `completed`). Checkpoints are written at stable boundaries (end of file or batch), not per chunk, to avoid persistence overhead.

If one document in a batch fails, classify and surface the error deterministically. When backend semantics support it, rollback partial write units; otherwise mark partials explicitly and checkpoint only committed state.

### Query streaming pipeline

`query_stream` resolves `thread_id`, loads latest checkpoint, performs retrieval, then runs the ReAct loop. Emit events in strict monotonic order by `step`:

1. `Status` (`retrieving`, `thinking`, `tool_call`, `answering`)
2. `Thought`
3. `ToolCall`
4. `Observation`
5. repeat as needed
6. `Final` or terminal `Error`

`step` increments per emitted semantic event, never decreases, and is unique within an execution stream.

### Backpressure and completion semantics

Use a bounded channel (default size 64, configurable). Never drop semantic events; only synthetic heartbeats may be skipped under pressure.

Adapter contract is deterministic:

- after `Final`: emit `status(completed)` then `done`
- after terminal `Error`: emit `error` then `done`

### Error layering

`RagError` wraps and chains lower-level errors (`IngestionError`, vector store errors, tool errors, checkpoint errors). Recoverable tool/runtime issues can emit `Error { recoverable: true }` and continue under policy. Fatal errors emit `Error { recoverable: false }` and terminate cleanly.

Checkpoint writes happen only after successful state transitions to prevent partial state persistence.

## 4) Testing Strategy and Acceptance Criteria

Testing targets migration safety end-to-end, not only crate-local correctness.

### Test layers

1. Contract tests (`AgentEvent`)
   - serde round-trip stability
   - monotonic unique `step`
   - consistent `thread_id`
   - `ToolCall.id` and `Observation.id` correlation
   - deterministic terminal semantics

2. Adapter conformance (`SimpleAgenticRagV1`)
   - exact SSE event names and payload fields
   - backpressure simulation ensures semantic events are never dropped
   - clean termination with `done`

3. Persistence and state parity
   - SQLite/Postgres save/load parity
   - resume by `thread_id`
   - projection on/off behavior
   - rollback on projection failure
   - no partial checkpoint guarantees

4. End-to-end migration tests
   - sample lifecycle: upload -> index -> search -> query -> stream -> resume
   - negative paths: unknown `thread_id`, interrupted query then resume, malformed file paths

### Milestone acceptance

- Week 1-2: runnable vertical slice (`wesichain-rag` + SQLite + SSE-compatible stream + resume)
- Week 3-4: Postgres parity + advanced event fidelity + recoverable error and backpressure hardening
- Week 5-6: ingestion parity defaults + reference app parity + migration toolkit docs

Performance is a directional guardrail in this milestone (bounded memory, bounded buffers, no unbounded stream growth), not a hard release gate.

## 5) Execution Timeline and Deliverables

### Wave 1 (Week 1-2): Vertical slice

- Add `AgentEvent` in `wesichain-core` with invariants and serde tests.
- Emit semantic events from graph/agent execution path.
- Add `wesichain-rag` facade with SQLite-backed session flow.
- Add `SimpleAgenticRagV1` SSE adapter.
- Deliver runnable example: `cargo run --example simple-rag-stream` (ingest -> stream answer -> resume by `thread_id`).

### Wave 2 (Week 3-4): Production hardening

- Postgres parity in facade with backend swap and no API changes.
- Retry and recoverability controls in facade runtime policy.
- Adapter/load tests under backpressure with deterministic completion.
- Deliver parity demo showing SQLite/Postgres switch by builder config.

### Wave 3 (Week 5-6): Migration completeness

- Ensure `txt/pdf/docx` and recursive splitter usage paths are first-class in facade defaults.
- Publish reference app mirroring the sample endpoint lifecycle.
- Publish migration toolkit docs:
  - concept mapping (Python graph workflows -> Wesichain)
  - endpoint mapping (sample routes -> facade calls)
  - streaming adapter recipes (SSE/WebSocket)
  - checkpoint migration recipes and session resumption
  - troubleshooting and observability patterns

## Risks and Contingency

Largest risk is hidden complexity in streaming fidelity and client-specific state shape. Mitigation is to lock event invariants and a runnable Wave 1 artifact early, then absorb compatibility adjustments in Wave 2 without breaking the facade API.

## Out of Scope for This Milestone

- Fully generic custom event registries in core.
- Multi-transport framework extraction into separate transport crates unless adapter demand appears.
- Hard perf SLO gates as release blockers.

## Decision

Proceed with Wave 1 immediately after this design commit, then move sequentially through Wave 2 and Wave 3 with client-runnable checkpoints at each wave boundary.
