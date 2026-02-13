# Wesichain LangSmith Compatibility Enhancement Design

Date: 2026-02-05
Status: Validated

## Goal
Deliver near-identical LangSmith observability parity with common Python stacks for core execution flows (Runnable, chains, LLMs, tools, graphs, agents) while preserving Wesichain's zero-overhead defaults and low runtime cost.

## Success Criteria
- Rust and Python versions of the same workflow produce comparable LangSmith run trees (inputs, outputs, errors, timing, parent/child hierarchy).
- Callback system is opt-in and introduces negligible overhead when disabled.
- LangSmith export pipeline is resilient (batching, retries, backpressure) and never blocks execution.
- Tracing overhead stays below 3-5% in loop benchmarks with observability enabled.

## Scope (MVP)
- Core run tree parity: inputs/outputs, errors, timing, run_type, parent/child IDs, trace_id.
- Callback-based instrumentation across Runnable, graph, agent, LLM, and tool calls.
- LangSmith handler with batching, sampling, redaction, and retries.

## Non-goals (MVP)
- LangSmith datasets/experiments/feedback endpoints.
- Full streaming fidelity (token-by-token updates).
- OpenTelemetry integration or non-LangSmith exporters.

## Approach Options
- Core callback manager (chosen): LangSmith-agnostic callbacks in `wesichain-core`, handlers in `wesichain-langsmith`.
- Tracing layer: relies on `tracing` spans; weaker control over inputs/outputs and run tree.
- Wrapper-only in LangSmith crate: minimal core changes but poor migration UX and incomplete coverage.

## Architecture Overview
Introduce a `wesichain-core::callbacks` module with:
- `RunType` enum (chain, llm, tool, graph, agent, retriever, runnable).
- `RunContext` (run_id, parent_run_id, trace_id, run_type, name, start_time, tags, metadata).
- `RunConfig` (callbacks, tags, metadata, name_override).
- `CallbackHandler` trait with async hooks: `on_start`, `on_end`, `on_error`, optional `on_stream_chunk`.
- `CallbackManager` that fans out to handlers and defaults to no-op.

Wrappers in core (`TracedRunnable`, `TracedTool`, `TracedLlm`) emit callback events without changing existing trait signatures. Entry points (graphs, agents) create root contexts and auto-wrap child calls when callbacks are present.

`wesichain-langsmith` implements `LangSmithCallbackHandler` and maps callback events to LangSmith Run create/update requests with batching and retries.

## Instrumentation and Data Flow
- Root run created at entry points (`graph`, `agent`, or `chain`), with `trace_id = run_id`.
- Child runs inherit `trace_id` and set `parent_run_id` via `RunContext::child`.
- Wrappers emit `on_start` with inputs, then `on_end` with outputs or `on_error` on failures.
- Default names: graph "graph_execution", LLM "llm_invoke", tool `tool.name`, runnable `node.name`.
- Streaming is optional and feature-gated; MVP does not emit incremental updates.

## LangSmith Handler and Export Pipeline
`LangSmithCallbackHandler` maps events to HTTP calls:
- `on_start` -> POST `/runs` with id, parent_run_id, trace_id, name, run_type, start_time, inputs, tags, metadata, session_name.
- `on_end` -> PATCH `/runs/{id}` with end_time, outputs, extra.duration_ms.
- `on_error` -> PATCH `/runs/{id}` with end_time, error, extra.duration_ms (terminal semantics).

Export pipeline:
- Bounded queue with drop-oldest backpressure.
- Async batch flush by size or interval.
- Retries with exponential backoff on 5xx/timeout/429; no retry on 4xx.
- PATCH 404 treated as success to avoid duplicate failure cascades.
- Redaction and truncation applied in handler before enqueue.
- Deterministic sampling by run_id; drop entire trace by trace_id to avoid orphaned children.

## Run Type Mapping
- Graph root: `graph` (or `chain` if LangSmith UI does not recognize `graph`).
- Graph nodes/runnables: `chain` by default; `runnable` reserved for generic wrappers if needed.
- LLM calls: `llm`.
- Tool calls: `tool`.
- Agent root: `agent`.

## Configuration
`LangSmithConfig` (in `wesichain-langsmith`):
- api_key, api_url, project_name/session_name
- flush_interval, max_batch_size, queue_capacity
- sampling_rate, redact_regex

`RunConfig` (in `wesichain-core`):
- callbacks: Option<CallbackManager>
- tags, metadata, name_override

## Testing and Validation
Core tests:
- RunContext propagation (trace_id and parent_run_id).
- CallbackManager noop behavior.
- Input/output wrapping to JSON objects.

Wrapper tests:
- Single `on_start`/`on_end` per success.
- `on_error` emitted before error propagation.
- Default names and RunConfig overrides.

LangSmith tests (wiremock):
- POST includes idempotency header and payload fields.
- PATCH is partial and preserves terminal errors.
- Sampling drops entire traces (no orphans).
- Backpressure drop-oldest behavior and counters.
- Retry behavior for 5xx/429 and no retry for 4xx.

Parity validation:
- Run identical Python and Rust workflows; compare LangSmith UI traces.

Performance:
- Benchmark with callbacks disabled (noise-level overhead).
- Benchmark with LangSmith enabled (<3-5% target).

## Rollout Plan (4-6 weeks)
Week 1-2: Core callbacks module + wrappers + basic handler mapping.
Week 3-4: Instrument graphs/agents/LLM/tool flows; add tests and wiremock coverage.
Week 5-6: Benchmarking, docs, migration example, release candidate.

## Risks and Mitigations
- Schema drift: pin fields to current LangSmith schema; track changelog.
- Overhead creep: keep callbacks optional, batch requests, minimize allocations.
- Partial traces: deterministic sampling by trace_id; drop full traces when unsampled.

## Open Questions
- Final `run_type` mapping for graph vs chain in LangSmith UI.
- Streaming event schema and whether to use PATCH outputs or a dedicated events array.

## Example (Rust)
```rust
use std::sync::Arc;
use wesichain_core::callbacks::{CallbackManager, RunConfig};
use wesichain_graph::{ExecutionOptions, GraphBuilder};
use wesichain_langsmith::{LangSmithConfig, LangSmithCallbackHandler};

let handler = Arc::new(LangSmithCallbackHandler::new(LangSmithConfig::from_env()?));
let callbacks = CallbackManager::new(vec![handler]);

let options = ExecutionOptions {
    run_config: Some(RunConfig::new(callbacks)),
    ..Default::default()
};

let graph = GraphBuilder::new()
    .add_node("agent", agent_node)
    .set_entry("agent")
    .build();

let _ = graph.invoke_with_options(state, options).await?;
```
