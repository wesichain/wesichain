# Observability/Tracing Design: TokenUsage, OTel Honesty, and Trait Unification

## 1. Problem Statement

Wesichain's observability stack has four interconnected issues requiring immediate attention:

| Issue | Severity | Description |
|-------|----------|-------------|
| Credibility gap | **Critical** | CLAUDE.md claims "OpenTelemetry-compatible tracing" that doesn't exist |
| Retention blocker | **High** | No structured `TokenUsage` means users can't track LLM costs or debug prompt regressions |
| Architecture debt | **Medium** | Dual traits (`CallbackHandler` + `Observer`) multiply implementation cost for each new backend |
| Latent bug | **Medium** | `TracedRunnable::stream()` doesn't fire callbacks, undermining existing LangSmith integration |

The credibility gap is most urgent: being caught overclaiming damages trust more than admitting a missing feature.

## 2. Immediate Actions

### 2.1 Remove OTel Claim from CLAUDE.md

Remove this line:
```markdown
- **LangSmith Compatibility**: Full observability integration via `LangSmithTracer` implementing callback traits,
  sending traces to LangSmith REST API with batching/background sending for minimal overhead.
```

Replace with:
```markdown
- **Observability**: Native `CallbackHandler` trait for custom backends; `wesichain-langsmith` crate provides
  production-ready LangSmith integration with batching, sampling, and PII redaction.
```

### 2.2 Add ROADMAP.md Entry

Create or update `ROADMAP.md`:
```markdown
## In Active Design

### OpenTelemetry Support
- Status: Design phase
- Scope: OTel SDK integration with span export, W3C traceparent propagation for distributed tracing
- Blockers: Trait unification (see below) must stabilize first
- Target: Post-v0.3, no committed timeline
```

This signals maturity without committing to a deadline.

## 3. TokenUsage Design: Structured LLM Observability

### 3.1 New Types

```rust
// wesichain-core/src/callbacks/llm.rs

/// Token consumption for cost tracking and optimization.
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// LLM call parameters captured at start time.
pub struct LlmInput {
    pub model: String,
    /// Rendered prompt (after template expansion), not the template itself
    pub prompt: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
}

/// LLM call results captured at end time.
pub struct LlmResult {
    pub token_usage: Option<TokenUsage>,
    pub model: String,
    pub finish_reason: Option<String>,
    /// Rendered output strings (one per generation)
    pub generations: Vec<String>,
}
```

### 3.2 Extended CallbackHandler Trait

```rust
#[async_trait]
pub trait CallbackHandler: Send + Sync {
    // Existing methods unchanged
    async fn on_start(&self, ctx: &RunContext, inputs: &Value);
    async fn on_end(&self, ctx: &RunContext, outputs: &Value, duration_ms: u128);
    async fn on_error(&self, ctx: &RunContext, error: &Value, duration_ms: u128);
    async fn on_stream_chunk(&self, _ctx: &RunContext, _chunk: &Value) {}

    // New structured LLM callbacks with default implementations for backward compatibility
    async fn on_llm_start(&self, ctx: &RunContext, input: &LlmInput) {
        self.on_start(ctx, &serde_json::to_value(input).unwrap_or_default()).await
    }

    async fn on_llm_end(&self, ctx: &RunContext, result: &LlmResult, duration_ms: u128) {
        self.on_end(ctx, &serde_json::to_value(result).unwrap_or_default(), duration_ms).await
    }
}
```

**Key design decisions:**

1. **Default implementations**: Existing backends (LangSmith) continue working without changes. Backends opt-in to structured data by overriding the new methods.

2. **Separate start/end**: LangSmith requires span opening at `on_llm_start` to capture prompt and parameters. Without this, latency accuracy and prompt debugging are impossible.

3. **Rendered prompt, not template**: The `prompt` field contains the final string sent to the LLM. Template tracking is a separate concern (can be added to `RunContext.metadata` if needed).

4. **`token_usage: Option<TokenUsage>`**: Not all providers report token counts. Making it `Option` forces backends to handle the missing case explicitly rather than defaulting to zeros.

### 3.3 LangSmith Integration Path

`LangSmithCallbackHandler` will override both new methods:

```rust
#[async_trait]
impl CallbackHandler for LangSmithCallbackHandler {
    // ... existing on_start/on_end for non-LLM runs ...

    async fn on_llm_start(&self, ctx: &RunContext, input: &LlmInput) {
        // Emit RunEvent::Start with LLM-specific run_type
        // Include temperature, max_tokens in metadata
    }

    async fn on_llm_end(&self, ctx: &RunContext, result: &LlmResult, duration_ms: u128) {
        // Emit RunEvent::Update with token_usage
        // Map finish_reason to LangSmith's expected format
    }
}
```

This gives LangSmith users the cost tracking and prompt debugging that makes the integration sticky.

## 4. Streaming Fix

**Bug**: `TracedRunnable::stream()` (in `wesichain-core/src/callbacks/wrappers.rs` line 70) passes through to `inner.stream()` without firing callbacks.

**Fix**: Implement streaming wrapper that:
1. Calls `on_start` on first poll
2. Calls `on_stream_chunk` for each `StreamEvent::Chunk`
3. Calls `on_end` (or `on_llm_end` if run_type is LLM) on successful completion
4. Calls `on_error` on failure

This is implementation work, not design—fix alongside the `on_llm_start/on_llm_end` addition.

## 5. Trait Unification: Constraint Definition

### 5.1 The Problem

Any new observability backend currently requires implementing **two traits**:
- `CallbackHandler` for core runnables (`wesichain-core`)
- `Observer` for graph nodes (`wesichain-graph`)

This doubles implementation surface and creates divergence risk (LangSmith integration differs between core and graph contexts).

### 5.2 The Constraint

The unified trait must satisfy:

1. **Superset coverage**: Include all methods from both `CallbackHandler` and `Observer`
2. **Default implementations**: All methods have defaults so existing implementors don't break
3. **Reference pattern**: `LangSmithCallbackHandler` becomes the reference—overrides only what it needs
4. **Backend-agnostic**: No LangSmith-specific types in the unified trait

### 5.3 Explicit Deferral

**Unification design will be finalized after the OTel crate design exercise.**

Rationale: OTel has strong opinions about span lifecycle (span contexts, parent-child relationships, attributes vs events) that will stress-test both existing traits simultaneously. Rather than speculating on the unified design now, let OTel requirements surface where the current dual-trait system breaks down cleanly.

This means:
- `wesichain-opentelemetry` will initially implement both `CallbackHandler` and `Observer`
- Pain points during that implementation will inform the unified trait
- Unification happens as a refactoring step, not a blocker for OTel

## 6. Implementation Sequencing

| Step | Work | Deliverable | Effort |
|------|------|-------------|--------|
| 1 | Remove OTel claim from CLAUDE.md | PR with doc fix | 1 hour |
| 2 | Add ROADMAP.md entry | Commit to main | 30 min |
| 3 | Define `LlmInput`, `LlmResult`, `TokenUsage` types | PR to `wesichain-core` | 2 hours |
| 4 | Add `on_llm_start/on_llm_end` to `CallbackHandler` | PR with default impls | 2 hours |
| 5 | Fix streaming callbacks in `TracedRunnable` | Bug fix PR | 4 hours |
| 6 | Update `LangSmithCallbackHandler` to use structured methods | Feature PR | 4 hours |
| 7 | Design `wesichain-opentelemetry` crate (dual-trait) | Design doc | 1 day |
| 8 | Implement OTel crate | Crate + tests | 2-3 days |
| 9 | Graceful shutdown flush | PR to `wesichain-langsmith` | 4 hours |
| 10 | Unified trait design | Design doc | 1 day |
| 11 | Langfuse crate | Post-unification | Future |

Steps 1-6 can proceed in parallel after step 4 (trait extension). Steps 7-8 explicitly use the dual-trait approach knowing unification comes later.

## 7. Open Questions

### W3C Traceparent Propagation
Distributed tracing requires threading `traceparent` header through `RunContext`. This touches:
- `RunContext` structure (add `traceparent: Option<String>`)
- HTTP-based runnables (propagate incoming headers)
- Graph node execution (pass context across async boundaries)

**Decision**: Defer to OTel crate design. OTel SDK has `Context` propagation utilities; design our integration to leverage them rather than reinventing.

### Langfuse Priority
Langfuse (open-source LangSmith alternative) is lower priority than it appears. Teams that need self-hosted observability are more tolerant of gaps than teams needing OTel compatibility for existing infrastructure.

**Decision**: After OTel crate ships. Langfuse integration is straightforward once the trait is unified and the OTel mapping layer exists as reference.

### Cost Calculation API
`TokenUsage` enables cost tracking, but pricing varies by provider and model. Should Wesichain provide a `CostCalculator` trait?

**Decision**: Not now. Start with raw token counts; cost calculation is a higher-level concern that can be built on top once usage patterns emerge.

---

**Status**: Design ready for implementation (steps 1-6).
**Next action**: Step 1 (CLAUDE.md fix) should be a standalone PR merged immediately to close the credibility gap.
