# Wesichain v0.3 Agent Runtime and Typed Tools Design

Date: 2026-02-18
Status: Validated
Author: OpenCode + bene

## Context

This design addresses the highest-friction gaps identified in the Python-to-Rust migration review:

- Agent creation requires hand-rolled loop logic (~100 LOC) with subtle state risks.
- Tool definitions are JSON-heavy and runtime-error-prone.
- Streaming is currently buffered in typical usage patterns.
- Performance claims for agentic workloads need reproducible evidence.
- Error handling policy for malformed model actions and retries is under-specified.

The friction table from the review is treated as the primary prioritization artifact.

## Problem Statement

Wesichain has strong core primitives but lacks a stable, high-level agent/runtime contract that:

1. Preserves Rust-grade correctness guarantees for agent loops.
2. Reduces migration friction for Python users without copying Python's dynamic failure modes.
3. Standardizes tool contracts and error surfaces.

## Scope and Priorities

### Must-Ship in v0.3

1. Agent runtime with typed finite-state-machine (FSM) semantics.
2. Typed tool API with schema generation from typed args/results (no macros in first cut).

### Stretch for v0.3 / Target for v0.4+

- First-class async streaming API (v0.3 ships streaming-ready event seams).
- Structured output convenience extraction APIs.

### Non-Goals for v0.3

- Full LangChain mental-model compatibility.
- Macro-first ergonomics (`#[derive(Tool)]`, attribute macros) before API stabilization.
- Resumable `Interrupted` contract at the agent API layer.

## Design Principles

1. Safety guarantees over migration speed.
2. Deterministic defaults over implicit magic.
3. Ergonomic wrappers must not bypass core invariants.
4. Runtime validation is mandatory even with strong typing.
5. Macros are optional sugar after the typed API is proven.

## Alternatives Considered

### A. FSM-first core (chosen)

- Typed runtime states, compile-time transition constraints, uniform policy handling.
- Moderate migration cost, strongest long-term correctness moat.

### B. Compatibility wrapper first

- Fast adoption and minimal user disruption.
- Risks preserving fragile loop behavior and inconsistent failure semantics.

### C. Macro-heavy ergonomics first

- Lowest initial user LOC.
- Higher compile complexity, poorer diagnostics, and premature API lock-in.

## Architecture Overview

v0.3 introduces a new `wesichain-agent` crate centered on explicit runtime components:

- `AgentRuntime<S, Tools, Policy>`: typestate FSM executor.
- `LlmAdapter`: provider-normalized completion boundary.
- `ToolRegistry`: typed-to-erased dispatch boundary.
- `PolicyEngine`: centralized retry/reprompt/fail decisions.
- `EventSink`: event emission boundary for sync consumption now, async stream later.

## FSM Model

### States

- `Idle`
- `Thinking`
- `Acting`
- `Observing`
- `Completed` (terminal)
- `Failed` (terminal)
- `Interrupted` (terminal, non-resumable in v0.3)

### Canonical Loop

`Idle -> Thinking -> (Completed | Acting) -> Observing -> Thinking ...`

Terminal transitions:

- Any active state can transition to `Failed` on unrecoverable error.
- Policy-directed halt transitions to `Interrupted`.

### Two-Layer Guarantee Model

#### Layer 1: Compile-time guarantees (typestate)

Phase-specific runtime types only expose legal methods:

- `Runtime<Idle>::think(...)`
- `Runtime<Thinking>::complete(...)` or `Runtime<Thinking>::act(...)`
- `Runtime<Acting>::observe(...)`

Illegal transition calls are unrepresentable and enforced by compile-time tests.

#### Layer 2: Runtime validation (external data)

LLM/tool outputs remain untrusted runtime inputs. The runtime validates response shape and dispatch conditions before transition.

## Core Contracts

### `AgentState` Contract (minimum)

```rust
pub trait AgentState {
    type FinalOutput;
    type ScratchpadEntry;
    type StepId: Copy + Eq + std::fmt::Debug;

    fn user_input(&self) -> &str;
    fn append_scratchpad(&mut self, entry: Self::ScratchpadEntry);
    fn set_final_output(&mut self, out: Self::FinalOutput);
    fn step_count(&self) -> u32;
    fn correlation_id(&self) -> &str;
}
```

Everything else is extension traits to keep the base contract small and stable.

### `LlmAdapter` Boundary

```rust
#[async_trait::async_trait]
pub trait LlmAdapter {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;
}
```

- `complete()` is the only stable method in v0.3.
- `stream_complete()` is reserved for v0.4+ to avoid immediate trait churn.
- Model-action validation (tool-call shape, unknown tools, arg shape) is owned by `AgentRuntime`, not `LlmAdapter`.

### `PolicyEngine` Boundary

```rust
pub trait PolicyEngine<S: AgentState> {
    fn on_model_error(&self, state: &S, error: &AgentError) -> PolicyDecision;
    fn on_tool_error(&self, state: &S, error: &AgentError) -> PolicyDecision;
}
```

Policy decisions are deterministic and side-effect-free from the runtime perspective.

### `PolicyDecision` Contract

```rust
pub enum PolicyDecision {
    Fail,
    Retry {
        consume_budget: bool,
    },
    Reprompt {
        strategy: RepromptStrategy,
        consume_budget: bool,
    },
    Interrupt,
}

pub enum RepromptStrategy {
    OnceWithToolCatalog,
    N { n: u32 },
}
```

Normative defaults:

- `Retry` and `Reprompt` consume step budget by default (`consume_budget = true`).
- Any opt-out is advanced-only and must be explicit in configuration.
- `RepromptStrategy::N` must be bounded (`n > 0`) and never unbounded.

Implementation lock-in note (2026-02-19):

- v0.3 implementation keeps `PolicyDecision` shape exactly as above.
- Cancellation checks are applied consistently at phase boundaries, including successful tool completion paths.

## Typed Tool API (No Macros First)

### Public Contract

```rust
#[async_trait::async_trait]
pub trait TypedTool {
    type Args: serde::de::DeserializeOwned + schemars::JsonSchema;
    type Output: serde::Serialize + schemars::JsonSchema;

    const NAME: &'static str;

    async fn run(&self, args: Self::Args, ctx: ToolContext) -> Result<Self::Output, ToolError>;
}
```

### `ToolContext` in v0.3

`ToolContext` must carry:

- `correlation_id`
- `step_id`
- `tokio_util::sync::CancellationToken`

Additional fields are additive in future versions.

### Tool Registration and Dispatch

- `ToolSet::new().register(T1).register(T2).build()?`
- Duplicate `NAME` values fail at `build()` time.
- Registry exports model-facing schema catalog from `schemars` JSON Schema.
- Runtime receives `ToolCallEnvelope { name, args, call_id }`, resolves tool by name, deserializes typed args, executes, and serializes typed output into canonical result envelopes.

### Internal Erasure Boundary

The runtime uses internal `ErasedTool` adapters for heterogeneous storage and dispatch. This is internal-only; user APIs remain typed.

### Tool Error Model

- `ToolError` is a trait-object boundary wrapped as `ToolExecutionError`:
  - `kind: ToolErrorKind` (machine-readable)
  - `message: String`
  - `source: Option<Box<dyn std::error::Error + Send + Sync>>`
- Dispatch maps errors to `ToolDispatchError::Execution` with preserved context.

## Failure and Retry Policy

### Malformed Model Action Policy

Unknown tool names, invalid tool arg payloads, and malformed call shapes map to:

`AgentError::InvalidModelAction { step_id, tool_name, received_args, raw_response }`

Default policy: `FailFast`.

Optional bounded policies:

- `RepromptOnceWithToolCatalog`
- `RepromptN { n }`

Unbounded retry is forbidden.

### Step Budget Semantics

Reprompts consume step budget by default.

- Advanced opt-out exists for specialized workloads.
- Opt-out must be explicitly configured and documented as risk-bearing.

### `AgentError` Categories

- `ModelTransport`
- `InvalidModelAction`
- `ToolDispatch`
- `BudgetExceeded`
- `PolicyConfigInvalid` (illegal/contradictory policy configuration)
- `PolicyRuntimeViolation` (runtime contract breach by policy handling path)
- `InternalInvariant`

`InternalInvariant` is non-panicking by default and must include diagnostic context.

## Event Model and Streaming Seam

Every transition emits `AgentEvent`:

- `StepStarted`
- `ModelResponded`
- `ToolDispatched`
- `ToolCompleted`
- `StepFailed`
- `Completed`

v0.3 ships stable synchronous event consumption plus an optional unstable adapter feature:

- Cargo feature: `wesichain-agent/unstable-streaming`

This preserves forward compatibility while preventing buffered-replay lock-in.

## Testing Strategy

### 1) Compile-time Typestate Tests

- `trybuild` suite for illegal transitions (`act()` from `Idle`, `complete()` from `Acting`, etc.).

### 2) Runtime Validation and Policy Tests

- Unknown tool name handling.
- Invalid JSON argument shape handling.
- Tool execution errors and policy outcomes.
- Budget exhaustion and deterministic terminal behavior.

### 3) Integration Loop Tests

- No-tool direct completion.
- Single tool hop.
- Multi-hop tool chain.
- Malformed response recovery under bounded reprompts.

### 4) Event Contract Tests

- Ordering invariant: `StepStarted` precedes terminal event for each step.
- Cardinality invariant: each `ToolDispatched` has exactly one completion/failure.

### 5) Chaos/Fault Injection Tests

- `LlmAdapter` transport failure at step boundaries.
- Mid-step cancellation token trigger in each phase (`Thinking`, `Acting`, `Observing`).
- Partial failure propagation under concurrent event observers.

## Benchmark Policy (Always-On)

Benchmarks are continuous engineering input, not milestone theater.

### Required Scenarios

- Short-answer, no-tools.
- Single-tool hop.
- Multi-hop tool chain.
- Malformed-response recovery path.

### Required Metrics

- p50 latency
- p95 latency
- peak memory
- steps-to-completion

### Release Gate Thresholds (v0.3)

- p50 latency regression > 7%: review required.
- p95 latency regression > 10%: merge blocked without explicit sign-off.
- peak memory regression > 5%: merge blocked without explicit sign-off.
- error/crash regression > 0%: merge blocked.

## Rollout and Migration

v0.3 allows moderate API breaks in agent/tool surfaces.

Required migration artifacts:

1. Old manual loop -> `AgentRuntime` mapping guide.
2. Manual `ToolSpec` JSON -> `TypedTool` migration guide.
3. Deprecation schedule for JSON-manual tool paths.

Compatibility wrappers may exist, but cannot bypass FSM validation or policy gates.

## Acceptance Criteria

1. Invalid FSM transitions are compile-time failures.
2. Manual JSON parsing is not required in standard tool implementations.
3. Malformed model actions produce deterministic, structured errors.
4. Retry/reprompt behavior is bounded and budget-accounted by default.
5. Event contract invariants pass under normal and injected-failure conditions.
6. Benchmark gates are active in CI with explicit thresholds.

## Rationale Snapshot

This design intentionally avoids a 1:1 Python abstraction port.

- Python parity informs ergonomics.
- Rust strengths drive correctness model.

The result is a migration-friendly API surface that treats compile-time and runtime safety as first-class product features, not implementation details.
