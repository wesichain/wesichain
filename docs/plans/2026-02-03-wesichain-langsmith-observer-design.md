# Wesichain LangSmith Observer Design

Date: 2026-02-03
Status: Draft

## Goal
Provide LangSmith-compatible observability for Phase 5 graph execution by implementing a graph-level Observer that streams node and tool runs to LangSmith with minimal overhead and no impact on agent execution.

## Scope
- Phase 5 Observer only (graph-level events: node start/end, tool call/result, error).
- Custom batch exporter with async flush to LangSmith Runs API.
- No native hooks for v0 chains or evaluations/datasets/feedback.

## Non-goals
- LLM spans without explicit observer hooks.
- Offline JSONL export (future option).
- LangSmith datasets, experiments, or feedback endpoints.

## Related Docs
- docs/plans/2026-02-01-wesichain-v0-design.md
- docs/plans/2026-02-03-wesichain-phase-5-react-tools-design.md

## Success Criteria
- A graph run appears in LangSmith with correct parent-child hierarchy.
- Tool calls show inputs and results.
- Errors surface as failed runs without blocking execution.
- Overhead remains under 5 percent for typical ReAct workloads.

## Architecture
Crate: wesichain-langsmith (depends on wesichain-graph, not the reverse).

Components:
- LangSmithObserver: implements Observer, normalizes graph events into RunEvent.
- LangSmithExporter: owns bounded queue, batching, retries, and background flush.
- LangSmithClient: HTTP transport (reqwest) for create/update runs.
- RunContextStore: maps local run ids to metadata and parent-child relationships.

Observer integration:
- Injected via ExecutionOptions at graph invocation time.
- Sampling happens in LangSmithObserver before serialization.
- Inputs/outputs are derived via explicit conversion traits, not full state dumps.

## Data Flow
1. Observer receives a graph event and consults Sampler using trace_id.
2. If sampled, Observer builds a RunEvent with sanitized inputs/outputs.
3. RunEvent is enqueued to a bounded mpsc channel.
4. Exporter batches events and flushes by size or interval.
5. Client POSTs runs on start and PATCHes runs on end/error.

Backpressure:
- Queue is bounded. When full, drop oldest events (ring buffer) and increment dropped_events.
- Never block graph execution.
- Dropped events are reported via tracing and a health accessor (not Observer::on_error).

## Run Lifecycle and Semantics
- on_node_start: POST /runs with start_time.
- on_tool_call: POST /runs as child with parent_run_id.
- on_node_end: PATCH /runs/{id} with outputs and end_time.
- on_error: PATCH /runs/{id} with error and end_time if missing.

Terminal rules:
- First terminal event sets status (Completed or Failed).
- Later terminal events do not clear error, but may add outputs.
- PATCH payloads are partial (only changed fields).

Tool concurrency:
- Tool runs under the same parent get unique run ids.
- RunContextStore must be thread-safe for future parallel tool calls.

## Wire Format and Endpoints
Endpoints (default api_url):
- Create: POST https://api.smith.service/runs
- Update: PATCH https://api.smith.service/runs/{run_id}

Runs payload fields:
- id (UUID v4)
- parent_run_id (optional)
- name (node id or tool name)
- run_type (chain, tool)
- start_time/end_time (RFC3339 UTC)
- inputs/outputs (objects)
- error (string, optional)

Idempotency:
- Include x-idempotency-key per POST request to avoid duplicate runs on retry.

Inputs/outputs shape:
- Always objects; wrap non-object values as { "value": <value> }.

## Serialization and Redaction
- Define LangSmithInputs and LangSmithOutputs traits per state type.
- Redaction regex applies before truncation.
- Truncate each field to 100KB to preserve schema shape.
- Avoid serializing large vectors or embeddings by default.

## Configuration and API
Public surface:
- LangSmithConfig { api_key, api_url, project_name, flush_interval, max_batch_size, queue_capacity, sampling_rate, redact_regex }
- LangSmithObserver::new(config)
- LangSmithObserver::with_sampler(sampler)
- LangSmithObserver::flush(timeout) -> Result<FlushStats, FlushError>
- LangSmithObserver::dropped_events() -> u64

Defaults:
- api_url = https://api.smith.service
- sampling_rate = 1.0 (Sampler::Always)

Security:
- api_key must be redacted in Debug output. Use secrecy::SecretString or a custom Debug impl.

Project selection:
- project_name is sent in payload and resolved server-side.
- Self-hosted deployments may require a project_id; support both via config or documented override.

## Error Handling and Retries
Retry policy:
- 5xx and timeouts: retry with exponential backoff (max 3 attempts).
- 429: retry with backoff, honor Retry-After if present.
- 4xx (auth/validation): no retry, drop batch and log error.
- 404 on PATCH: warn and treat as success if create likely succeeded.

Flush errors:
- FlushError::Timeout { waited, pending }
- FlushError::Permanent { reason, batch_dropped }

Never log the api_key on failures.

## Testing
Unit tests:
- Sampling by trace_id (consistent per trace).
- Redaction before truncation; per-field 100KB cap.
- Inputs/outputs object wrapping.
- Terminal event ordering (error vs end).
- Dropped events counter and health reporting.

Integration tests (wiremock):
- Create run, update outputs, update error.
- Verify idempotency header on POST.
- Verify PATCH is partial and does not clear fields.

Test isolation:
- Use wiremock::MockServer on random ports.
- Use tempfile for disk-based tests.
- Use Uuid::new_v4() for trace ids.

Meta-observability:
- dropped_events and flush stats reflect queue behavior under load.

## Examples

Minimal graph integration:

```rust,no_run
use std::sync::Arc;
use std::time::Duration;
use wesichain_graph::{ExecutionOptions, GraphBuilder};
use wesichain_langsmith::{LangSmithConfig, LangSmithObserver};

let config = LangSmithConfig {
    api_key: std::env::var("LANGSMITH_API_KEY").expect("key required").into(),
    api_url: "https://api.smith.service".into(),
    project_name: "wesichain-prod".into(),
    flush_interval: Duration::from_secs(2),
    max_batch_size: 50,
    queue_capacity: 1000,
    sampling_rate: 1.0,
    redact_regex: None,
};

let observer = Arc::new(LangSmithObserver::new(config));
let options = ExecutionOptions {
    observer: Some(observer.clone()),
    ..Default::default()
};

let graph = GraphBuilder::new()
    .add_node("react_agent", react_node)
    .build();

let _ = graph.invoke_with_options(state, options).await?;

let _stats = observer.flush(Duration::from_secs(5)).await?;
```

Migration story:

```python
# Python (graph workflow)
# handler = LangSmithCallbackHandler(...)
# graph.invoke(state, config={"callbacks": [handler]})
```

```rust,no_run
// Rust (Wesichain)
let observer = Arc::new(LangSmithObserver::new(config));
let options = ExecutionOptions { observer: Some(observer), ..Default::default() };
graph.invoke_with_options(state, options).await?;
```

## Rollout
- Release wesichain-langsmith as 0.0.x experimental.
- Keep Observer integration opt-in via ExecutionOptions.
- Expand to LLM spans once ToolCallingLlm events are exposed.

## Future Work
- Optional JSONL exporter for offline environments.
- LangSmith evaluations/datasets/feedback endpoints (v0.6+).
