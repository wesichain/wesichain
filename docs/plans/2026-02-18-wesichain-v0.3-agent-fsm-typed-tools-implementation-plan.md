# Wesichain v0.3 Agent Runtime + Typed Tools Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement an FSM-first `wesichain-agent` runtime and typed tool API that removes manual loop/tool boilerplate while enforcing compile-time transition safety and deterministic runtime validation.

**Architecture:** Add a new `wesichain-agent` crate with typestate runtime phases (`Idle`, `Thinking`, `Acting`, `Observing`, terminal states), a pure `PolicyEngine`, and a typed tool registry (`TypedTool` + `ToolSet`) behind an erased internal dispatch boundary. Keep streaming API unstable in v0.3 but emit stable internal events on every transition.

**Tech Stack:** Rust 1.75+, async-trait, tokio, serde/serde_json, schemars, thiserror, criterion, trybuild.

---

## Prerequisites

- Approved design: `docs/plans/2026-02-18-wesichain-v0.3-agent-fsm-typed-tools-design.md`
- Worktree: `/Users/bene/Documents/bene/python/rechain/wesichain/.worktrees/v03-agent-runtime`
- Note: workspace-wide tests currently fail on this machine due disk exhaustion (`os error 28`). Use crate-scoped tests during development.

---

### Task 1: Scaffold `wesichain-agent` crate and workspace wiring

**Files:**
- Create: `wesichain-agent/Cargo.toml`
- Create: `wesichain-agent/src/lib.rs`
- Modify: `Cargo.toml`

**Step 1: Write the failing integration test reference first**

Create `wesichain-agent/tests/smoke.rs`:

```rust
#[test]
fn crate_exports_public_entrypoints() {
    let _ = std::any::type_name::<wesichain_agent::AgentRuntime<(), (), wesichain_agent::NoopPolicy, wesichain_agent::Idle>>();
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-agent --test smoke -v`
Expected: FAIL with `package ID specification 'wesichain-agent' did not match any packages`.

**Step 3: Add minimal crate + workspace entry**

```toml
# Cargo.toml (workspace)
[workspace]
members = [
  # ...existing members...
  "wesichain-agent",
]
```

```toml
# wesichain-agent/Cargo.toml
[package]
name = "wesichain-agent"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
wesichain-core = { path = "../wesichain-core", version = "0.2.1" }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "0.8"
thiserror = "1"
tokio = { version = "1", features = ["sync", "time"] }

[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
trybuild = "1"
criterion = "0.5"
```

```rust
// wesichain-agent/src/lib.rs
pub struct AgentRuntime<S, T, P, Phase> {
    _marker: std::marker::PhantomData<(S, T, P, Phase)>,
}

pub struct Idle;
pub struct NoopPolicy;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-agent --test smoke -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add Cargo.toml wesichain-agent/Cargo.toml wesichain-agent/src/lib.rs wesichain-agent/tests/smoke.rs
git commit -m "feat(agent): scaffold new agent runtime crate"
```

---

### Task 2: Define core runtime domain contracts

**Files:**
- Create: `wesichain-agent/src/state.rs`
- Create: `wesichain-agent/src/error.rs`
- Create: `wesichain-agent/src/policy.rs`
- Create: `wesichain-agent/src/event.rs`
- Modify: `wesichain-agent/src/lib.rs`
- Test: `wesichain-agent/tests/contracts.rs`

**Step 1: Write failing tests for contract shape**

```rust
use wesichain_agent::{AgentError, AgentEvent, PolicyDecision, RepromptStrategy};

#[test]
fn policy_decision_has_required_variants() {
    let _ = PolicyDecision::Fail;
    let _ = PolicyDecision::Retry { consume_budget: true };
    let _ = PolicyDecision::Reprompt {
        strategy: RepromptStrategy::OnceWithToolCatalog,
        consume_budget: true,
    };
    let _ = PolicyDecision::Interrupt;
}

#[test]
fn invalid_model_action_carries_debug_payload() {
    let err = AgentError::InvalidModelAction {
        step_id: 2,
        tool_name: Some("calculator".to_string()),
        received_args: serde_json::json!({"bad": true}),
        raw_response: serde_json::json!({"tool_calls": []}),
    };
    assert!(err.to_string().contains("Invalid model action"));
}

#[test]
fn agent_event_has_step_started_and_completed() {
    let start = AgentEvent::StepStarted { step_id: 1 };
    let done = AgentEvent::Completed { step_id: 1 };
    assert_ne!(format!("{start:?}"), format!("{done:?}"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-agent --test contracts -v`
Expected: FAIL with unresolved imports/variants.

**Step 3: Add minimal domain types**

Implement:

- `AgentError` variants:
  - `ModelTransport`
  - `InvalidModelAction { step_id, tool_name, received_args, raw_response }`
  - `ToolDispatch`
  - `BudgetExceeded`
  - `PolicyConfigInvalid`
  - `PolicyRuntimeViolation`
  - `InternalInvariant`
- `PolicyDecision` + `RepromptStrategy` exactly as design doc.
- `AgentEvent` with at least: `StepStarted`, `ModelResponded`, `ToolDispatched`, `ToolCompleted`, `StepFailed`, `Completed`.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-agent --test contracts -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-agent/src/state.rs wesichain-agent/src/error.rs wesichain-agent/src/policy.rs wesichain-agent/src/event.rs wesichain-agent/src/lib.rs wesichain-agent/tests/contracts.rs
git commit -m "feat(agent): add runtime contracts and policy domain types"
```

---

### Task 3: Implement `AgentState` trait and typestate phases

**Files:**
- Create: `wesichain-agent/src/phase.rs`
- Modify: `wesichain-agent/src/state.rs`
- Modify: `wesichain-agent/src/lib.rs`
- Test: `wesichain-agent/tests/agent_state.rs`

**Step 1: Write failing trait-bound test**

```rust
use wesichain_agent::AgentState;

#[derive(Default)]
struct DemoState {
    input: String,
    steps: u32,
    corr: String,
}

impl AgentState for DemoState {
    type FinalOutput = String;
    type ScratchpadEntry = String;
    type StepId = u32;

    fn user_input(&self) -> &str { &self.input }
    fn append_scratchpad(&mut self, _entry: Self::ScratchpadEntry) {}
    fn set_final_output(&mut self, _out: Self::FinalOutput) {}
    fn step_count(&self) -> u32 { self.steps }
    fn correlation_id(&self) -> &str { &self.corr }
}

#[test]
fn agent_state_contract_compiles() {
    let s = DemoState::default();
    assert_eq!(s.step_count(), 0);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-agent --test agent_state -v`
Expected: FAIL with missing `AgentState` trait methods.

**Step 3: Implement trait + phase markers**

Add:

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

pub struct Idle;
pub struct Thinking;
pub struct Acting;
pub struct Observing;
pub struct Completed;
pub struct Failed;
pub struct Interrupted;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-agent --test agent_state -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-agent/src/state.rs wesichain-agent/src/phase.rs wesichain-agent/src/lib.rs wesichain-agent/tests/agent_state.rs
git commit -m "feat(agent): add AgentState trait and typestate phase markers"
```

---

### Task 4: Add compile-time typestate guardrails with `trybuild`

**Files:**
- Create: `wesichain-agent/tests/typestate_compile.rs`
- Create: `wesichain-agent/tests/ui/invalid_act_from_idle.rs`
- Create: `wesichain-agent/tests/ui/invalid_complete_from_acting.rs`
- Create: `wesichain-agent/tests/ui/valid_think_act_observe.rs`
- Create: `wesichain-agent/src/runtime.rs`
- Modify: `wesichain-agent/src/lib.rs`

**Step 1: Write failing compile tests**

```rust
// wesichain-agent/tests/typestate_compile.rs
#[test]
fn typestate_rules() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_act_from_idle.rs");
    t.compile_fail("tests/ui/invalid_complete_from_acting.rs");
    t.pass("tests/ui/valid_think_act_observe.rs");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-agent --test typestate_compile -v`
Expected: FAIL because runtime transition methods are not implemented yet.

**Step 3: Implement minimal phase-specific runtime API**

Implement methods only on legal phases:

- `AgentRuntime<..., Idle>::think()`
- `AgentRuntime<..., Thinking>::act()`
- `AgentRuntime<..., Thinking>::complete()`
- `AgentRuntime<..., Acting>::observe()`

Do not implement `act()` for `Idle`.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-agent --test typestate_compile -v`
Expected: PASS with compile-fail fixtures accepted.

**Step 5: Commit**

```bash
git add wesichain-agent/src/runtime.rs wesichain-agent/src/lib.rs wesichain-agent/tests/typestate_compile.rs wesichain-agent/tests/ui/
git commit -m "feat(agent): enforce FSM transitions with typestate compile tests"
```

---

### Task 5: Implement `LlmAdapter` and runtime-owned validation boundary

**Files:**
- Create: `wesichain-agent/src/llm.rs`
- Create: `wesichain-agent/src/validation.rs`
- Modify: `wesichain-agent/src/runtime.rs`
- Modify: `wesichain-agent/src/lib.rs`
- Test: `wesichain-agent/tests/llm_boundary.rs`

**Step 1: Write failing boundary tests**

```rust
use wesichain_agent::{AgentError, LlmAdapter, ModelAction};

#[test]
fn runtime_not_adapter_marks_unknown_tool_as_invalid_model_action() {
    let err = AgentError::InvalidModelAction {
        step_id: 1,
        tool_name: Some("missing_tool".to_string()),
        received_args: serde_json::json!({}),
        raw_response: serde_json::json!({"tool_calls": [{"name": "missing_tool"}]}),
    };
    assert!(err.to_string().contains("Invalid model action"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-agent --test llm_boundary -v`
Expected: FAIL with missing adapter/action types.

**Step 3: Implement adapter + validation split**

Add:

```rust
#[async_trait::async_trait]
pub trait LlmAdapter {
    async fn complete(&self, request: wesichain_core::LlmRequest)
        -> Result<wesichain_core::LlmResponse, wesichain_core::WesichainError>;
}
```

And validation helper in runtime layer that converts raw response into `ModelAction` or `AgentError::InvalidModelAction`.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-agent --test llm_boundary -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-agent/src/llm.rs wesichain-agent/src/validation.rs wesichain-agent/src/runtime.rs wesichain-agent/src/lib.rs wesichain-agent/tests/llm_boundary.rs
git commit -m "feat(agent): add llm adapter boundary and runtime validation"
```

---

### Task 6: Add typed tool contracts and registry builder

**Files:**
- Create: `wesichain-agent/src/tooling.rs`
- Modify: `wesichain-agent/src/lib.rs`
- Test: `wesichain-agent/tests/tooling_registry.rs`

**Step 1: Write failing tests for duplicate detection and schema export**

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wesichain_agent::{ToolSet, TypedTool};

#[derive(Debug, Deserialize, JsonSchema)]
struct Args { expression: String }

#[derive(Debug, Serialize, JsonSchema)]
struct Out { value: f64 }

struct Calc;

#[async_trait::async_trait]
impl TypedTool for Calc {
    type Args = Args;
    type Output = Out;
    const NAME: &'static str = "calculator";
    async fn run(&self, _args: Self::Args, _ctx: wesichain_agent::ToolContext) -> Result<Self::Output, wesichain_agent::ToolError> {
        Ok(Out { value: 4.0 })
    }
}

#[test]
fn duplicate_tool_name_fails_build() {
    let err = ToolSet::new().register(Calc).register(Calc).build().unwrap_err();
    assert!(err.to_string().contains("duplicate"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-agent --test tooling_registry -v`
Expected: FAIL with missing tooling APIs.

**Step 3: Implement typed tooling surface**

Implement:

- `TypedTool` trait (`Args: DeserializeOwned + JsonSchema`, `Output: Serialize + JsonSchema`)
- `ToolContext { correlation_id, step_id, cancellation }`
- `ToolSet::new().register(...).build()` duplicate-name validation
- schema catalog export derived from `schemars`

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-agent --test tooling_registry -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-agent/src/tooling.rs wesichain-agent/src/lib.rs wesichain-agent/tests/tooling_registry.rs
git commit -m "feat(agent): add typed tool trait and registry builder"
```

---

### Task 7: Implement tool dispatch pipeline and structured dispatch errors

**Files:**
- Modify: `wesichain-agent/src/tooling.rs`
- Modify: `wesichain-agent/src/error.rs`
- Test: `wesichain-agent/tests/tool_dispatch.rs`

**Step 1: Write failing dispatch tests**

```rust
use wesichain_agent::{ToolDispatchError, ToolEnvelope};

#[test]
fn unknown_tool_maps_to_dispatch_error() {
    let err = ToolDispatchError::UnknownTool { name: "nope".to_string() };
    assert!(err.to_string().contains("nope"));
}

#[test]
fn invalid_args_maps_to_dispatch_error() {
    let err = ToolDispatchError::InvalidArgs {
        name: "calculator".to_string(),
        source: "missing expression".to_string(),
    };
    assert!(err.to_string().contains("Invalid tool args"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-agent --test tool_dispatch -v`
Expected: FAIL with missing dispatch error variants.

**Step 3: Implement dispatch and mappings**

Implement dispatch flow:

`ToolEnvelope { name, args, call_id } -> lookup -> deserialize -> run -> serialize`

And error enum:

- `UnknownTool`
- `InvalidArgs`
- `Execution`
- `Serialization`

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-agent --test tool_dispatch -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-agent/src/tooling.rs wesichain-agent/src/error.rs wesichain-agent/tests/tool_dispatch.rs
git commit -m "feat(agent): add typed dispatch pipeline and error mapping"
```

---

### Task 8: Implement runtime loop with policy + budget accounting

**Files:**
- Modify: `wesichain-agent/src/runtime.rs`
- Modify: `wesichain-agent/src/policy.rs`
- Test: `wesichain-agent/tests/runtime_loop.rs`

**Step 1: Write failing loop tests**

```rust
#[tokio::test]
async fn reprompt_consumes_budget_by_default() {
    // fixture policy emits Reprompt { consume_budget: true }
    // fixture runtime budget=2 must terminate as BudgetExceeded on repeated malformed actions
    let result = wesichain_agent::testkit::run_reprompt_budget_fixture(2).await;
    assert!(matches!(result.unwrap_err(), wesichain_agent::AgentError::BudgetExceeded { .. }));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-agent --test runtime_loop -v`
Expected: FAIL because loop/policy plumbing not complete.

**Step 3: Implement minimal loop semantics**

Requirements:

- Transition order: `Idle -> Thinking -> (Acting|Completed) -> Observing -> Thinking`
- `PolicyEngine` decides on model/tool errors
- reprompt/retry consume budget by default
- `Interrupted` is terminal and non-resumable in v0.3

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-agent --test runtime_loop -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-agent/src/runtime.rs wesichain-agent/src/policy.rs wesichain-agent/tests/runtime_loop.rs
git commit -m "feat(agent): implement fsm runtime loop with budget-aware policy"
```

---

### Task 9: Add event contract tests (ordering + cardinality)

**Files:**
- Modify: `wesichain-agent/src/event.rs`
- Modify: `wesichain-agent/src/runtime.rs`
- Test: `wesichain-agent/tests/event_contract.rs`

**Step 1: Write failing contract tests**

```rust
#[tokio::test]
async fn step_started_precedes_terminal_event() {
    let events = wesichain_agent::testkit::run_single_step_capture_events().await;
    let first = events.first().expect("at least one event");
    assert!(matches!(first, wesichain_agent::AgentEvent::StepStarted { .. }));
}

#[tokio::test]
async fn each_tool_dispatched_has_one_completion_or_failure() {
    let events = wesichain_agent::testkit::run_tool_capture_events().await;
    wesichain_agent::testkit::assert_tool_cardinality(events);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-agent --test event_contract -v`
Expected: FAIL before event sequencing is enforced.

**Step 3: Implement event emission guarantees**

In runtime:

- emit `StepStarted` before model/tool actions
- emit exactly one terminal event per step (`ToolCompleted` or `StepFailed`)
- emit `Completed` only once

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-agent --test event_contract -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-agent/src/event.rs wesichain-agent/src/runtime.rs wesichain-agent/tests/event_contract.rs
git commit -m "test(agent): enforce event ordering and cardinality invariants"
```

---

### Task 10: Add chaos/fault-injection coverage

**Files:**
- Create: `wesichain-agent/tests/chaos_failures.rs`
- Modify: `wesichain-agent/src/runtime.rs`

**Step 1: Write failing chaos tests**

```rust
#[tokio::test]
async fn llm_transport_failure_surfaces_as_model_transport() {
    let err = wesichain_agent::testkit::run_transport_fail_fixture().await.unwrap_err();
    assert!(matches!(err, wesichain_agent::AgentError::ModelTransport { .. }));
}

#[tokio::test]
async fn cancellation_during_acting_transitions_to_interrupted() {
    let out = wesichain_agent::testkit::run_cancel_during_acting_fixture().await.unwrap();
    assert!(matches!(out.terminal_state(), wesichain_agent::TerminalState::Interrupted));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-agent --test chaos_failures -v`
Expected: FAIL before cancellation/transport mapping is implemented.

**Step 3: Implement fault mapping + cancellation checks at phase boundaries**

Add explicit cancellation probes for:

- before `Thinking`
- before tool dispatch in `Acting`
- before `Observing` append

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-agent --test chaos_failures -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-agent/src/runtime.rs wesichain-agent/tests/chaos_failures.rs
git commit -m "test(agent): add fault-injection coverage for transport and cancellation"
```

---

### Task 11: Add agent runtime benchmarks and release gates

**Files:**
- Create: `wesichain-agent/benches/runtime_profiles.rs`
- Modify: `wesichain-agent/Cargo.toml`
- Create: `tools/bench/evaluate_agent_thresholds.py`
- Create: `tools/bench/agent-thresholds.toml`
- Modify: `.github/workflows/nightly-bench.yml`
- Modify: `.github/workflows/pr-checks.yml`

**Step 1: Write failing benchmark gate test input**

Create synthetic metrics JSON fixture in `tools/bench/tests/agent_metrics_sample.json` and a small script smoke check in CI command section.

**Step 2: Run script to verify it fails before implementation**

Run: `python3 tools/bench/evaluate_agent_thresholds.py --metrics-json tools/bench/tests/agent_metrics_sample.json --thresholds tools/bench/agent-thresholds.toml`
Expected: FAIL because script/threshold file does not exist.

**Step 3: Implement benchmark + thresholds**

Benchmark scenarios:

- no-tool short answer
- single-tool hop
- multi-hop chain
- malformed-response recovery

Thresholds (v0.3):

- p50 regression > 7% => review
- p95 regression > 10% => block
- peak memory regression > 5% => block
- error/crash regression > 0% => block

**Step 4: Run benchmark gate script to verify success**

Run:
- `cargo bench -p wesichain-agent --bench runtime_profiles -- --sample-size 10`
- `python3 tools/bench/evaluate_agent_thresholds.py --thresholds tools/bench/agent-thresholds.toml --criterion-root target/criterion --rss-file target/bench/rss-agent.txt --dataset-size 1000`

Expected: PASS on local baseline.

**Step 5: Commit**

```bash
git add wesichain-agent/benches/runtime_profiles.rs wesichain-agent/Cargo.toml tools/bench/evaluate_agent_thresholds.py tools/bench/agent-thresholds.toml .github/workflows/nightly-bench.yml .github/workflows/pr-checks.yml
git commit -m "perf(agent): add runtime benchmark profiles and ci threshold gates"
```

---

### Task 12: Documentation + migration updates

**Files:**
- Modify: `README.md`
- Modify: `ROADMAP.md`
- Modify: `docs/skills/react-agents.md`
- Create: `docs/migration/langchain-to-wesichain-agent-runtime.md`
- Modify: `docs/plans/2026-02-18-wesichain-v0.3-agent-fsm-typed-tools-design.md`

**Step 1: Write failing docs assertion check**

Run:

`grep -n "Python parity informs ergonomics, Rust strengths drive correctness model" README.md`

Expected: no match.

**Step 2: Add docs updates**

- Add mission line to README.
- Mark v0.3 in roadmap as active implementation track.
- Update React skill docs to include new `wesichain-agent` path and typed tools.
- Add migration doc mapping:
  - manual loop -> `AgentRuntime`
  - manual `ToolSpec` JSON -> `TypedTool` + `ToolSet`
- Keep design doc Status unchanged; include final `PolicyDecision` section (already approved).

**Step 3: Verify docs**

Run:

- `grep -n "Rust strengths drive correctness model" README.md`
- `grep -n "wesichain-agent" docs/skills/react-agents.md`
- `grep -n "ToolSet" docs/migration/langchain-to-wesichain-agent-runtime.md`

Expected: all commands return matching lines.

**Step 4: Commit docs**

```bash
git add README.md ROADMAP.md docs/skills/react-agents.md docs/migration/langchain-to-wesichain-agent-runtime.md docs/plans/2026-02-18-wesichain-v0.3-agent-fsm-typed-tools-design.md
git commit -m "docs: publish v0.3 agent runtime migration and positioning"
```

**Step 5: Tag completion checkpoint commit**

```bash
git tag -a v0.3-agent-runtime-plan-complete -m "v0.3 agent runtime + typed tool implementation complete"
```

---

## Verification Checklist

- [ ] `wesichain-agent` crate exists and is part of workspace.
- [ ] Typestate compile-fail tests pass (`trybuild`).
- [ ] `AgentState`, `PolicyDecision`, and `AgentError` contracts match approved design.
- [ ] Typed tool registration/dispatch removes manual JSON parsing from user code.
- [ ] Unknown tool + malformed args map to deterministic `InvalidModelAction`/dispatch errors.
- [ ] Reprompts consume budget by default.
- [ ] Event ordering/cardinality contract tests pass.
- [ ] Chaos tests cover transport failure + cancellation at phase boundaries.
- [ ] Benchmark gates are active with explicit thresholds.
- [ ] Migration docs and README language reflect FSM-first positioning.

## Execution Notes

- Keep commits small (one task per commit).
- Run crate-scoped commands during development to avoid machine-level disk failures from full workspace test runs.
- Only run workspace-wide checks after local disk pressure is resolved.
