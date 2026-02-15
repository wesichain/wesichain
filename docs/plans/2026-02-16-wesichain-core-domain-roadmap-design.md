# Wesichain Core Domain Roadmap Design

Date: 2026-02-16
Status: Validated

## Context and Baseline

This design addresses core-domain migration gaps with an accelerated parity strategy and strict execution discipline.

Current baseline already includes:
- Chroma vector store integration in `wesichain-chroma`.
- DOCX ingestion in `wesichain-retrieval` async loader dispatch.

These are treated as quality baselines for all new integrations.

## Goals

- Unblock the highest-frequency LangChain-style migrations within 12 weeks.
- Preserve stable core contracts while scaling connector count.
- Keep delivery velocity high without sacrificing production confidence.
- Build defensible performance claims with reproducible benchmarks.

## Non-goals (This 12-Week Window)

- Building custom auth, rate limiting, observability, or deployment platforms.
- Large differentiator projects that threaten parity throughput.
- Breaking Layer-1 core trait signatures.

## Strategy

Adopt a 90/10 operating model:
- 90% parity lane for migration blockers.
- 10% seed lane for one scoped differentiator exploration.

Primary user-outcome metrics:
- Closed issues labeled migration unblocked.
- Time-to-first-working-RAG for migrated projects.

## 12-Week Sequencing

### Weeks 1-2: Qdrant Vector Store (Slice 1)

- New crate: `wesichain-qdrant`.
- Optional API key default with cloud URL warning when missing.
- Deliver full four-artifact slice and reach merged + Green nightly.
- End state: scoreboard row transitions from WIP to DONE.

### Weeks 3-4: Weaviate Vector Store (Slice 2)

- New crate: `wesichain-weaviate`.
- Weaviate-specific GraphQL filter path translation and class/schema behavior.
- Reuse Qdrant slice pattern with Weaviate deltas only.

### Weeks 5-6: Cohere Embeddings (Slice 3)

- Add feature-gated provider module in `wesichain-embeddings`.
- Preserve shared provider patterns and core `Embedding` compatibility.

### Weeks 7-8: Loader Mini-Slices (Slice 4)

Separate mini-slices per format:
- CSV first, JSON second, Markdown third.
- Shared dispatcher in `wesichain-retrieval/src/loader.rs` remains the single ingest entry point.

Locked loader defaults:
- CSV: one document per row.
- JSON: top-level array => one document per element; top-level object => one document per file.
- Markdown: one document per file, raw markdown preserved.
- Markdown frontmatter: ignored for now.

### Weeks 9-10: pgvector Vector Store (Slice 5)

- New crate: `wesichain-pgvector`.
- Follow same builder/error/contract pattern as prior vector store slices.

### Weeks 11-12: Voyage Embeddings + Parity Polish (Slice 6)

- Add feature-gated Voyage provider.
- Complete migration docs hardening and error-message audit.
- Keep seed lane capped; pause seed work automatically if parity misses target.

## Architecture Decisions

### A1. Vertical Slice Delivery Model

Each integration is a shippable vertical slice with exactly four required artifacts:
1. Adapter implementation.
2. Contract/integration tests.
3. Migration parity example and guide.
4. Benchmark artifact with pinned baseline metadata.

No slice is complete without all four artifacts.

### A2. Crate Organization

- Vector stores ship as dedicated crates (dependency isolation).
- Embeddings providers ship in `wesichain-embeddings` behind feature flags.
- Loaders remain centralized behind retrieval dispatcher APIs.

### A3. Standardized Data Flow

Ingestion pipeline:
- `load -> split -> embed -> add`

Retrieval pipeline:
- `query -> embed -> search -> rerank (optional)`

This flow must stay stable across all parity slices.

### A4. Two-Layer Contract Model

Layer 1 (frozen for 12 weeks):
- Stable base traits and signatures in core.

Layer 2 (additive):
- Optional extension traits (for example scored search, MMR, hybrid search).

Capability model:
- Runtime discovery through a `Capabilities`-style trait.
- Compile-time guarantees through explicit extension trait bounds.

### A5. Error Handling Model

- Keep crate-local typed errors for backend-specific semantics.
- Map to core error types with structured context.
- Preserve source chains for debugging.
- Include fields needed for operations (backend, operation, retryability semantics).

## Governance

### G1. Release State Machine

- Green: all required checks pass, no expired waivers.
- Yellow: one active non-crash waiver with owner and <= 4-week expiry.
- Red: crash/error regression, expired waiver, or repeated threshold breach.

Only Green and Yellow are releasable. Red blocks release tags.

### G2. Cadence

- Weekly rhythm:
  - Mon: scope and impact-map confirmation.
  - Tue-Thu: vertical slice implementation.
  - Fri: hardening (docs/examples/scoreboard WIP updates).
- Monthly release anchor: last Thursday UTC.
- Slice completion does not auto-trigger a release tag.

### G3. Roles Per Sprint

- Parity lane owner.
- Benchmark steward.
- Migration-doc owner.

### G4. Required PR Notes

Each slice PR must include:
- Migration delta note.
- Rollback note.

## Benchmark and Validation Policy

### V1. Two-Tier Benchmarking

PR tier (advisory, non-blocking):
- Fast, touched-crate scoped checks.
- Reports deltas without blocking on performance regressions.
- Blocks only on benchmark execution failures.

Nightly tier (required, release-gating):
- Full execution matrix and threshold enforcement.
- Includes LangChain baseline comparison for externally defensible claims.

### V2. Release-Blocking Thresholds

- Query p50 regression > 10%.
- Query p95 regression > 15%.
- Query p99 regression > 25%.
- Index throughput regression > 20%.
- Peak memory regression > 30%.
- Error/crash regression > 0%.

### V3. Waivers

- Allowed only for non-crash regressions.
- Must include owner, reason, linked issue, and <= 4-week expiry.
- Expired waiver escalates state to Red unless explicitly re-approved.

## CI Targeting and Escalation

Default PR compile scope:
- Touched crates only.

Escalation rules:
- Core trait file changes fan out to all connector examples.
- Shared contract-impacting paths fan out to known dependents.
- Workspace/CI meta changes escalate to broader workspace checks.

Implementation uses an impact map config (for example `tools/ci/impact-map.toml`) to keep fan-out logic explicit and auditable.

## Examples and Migration Activation

Canonical migration examples:
- Live in each owning crate `examples/` directory.
- Docs index these examples rather than duplicating logic.

CI policy:
- PR: compile touched-crate examples.
- Nightly: run examples end-to-end.

Scoreboard transition rule:
- WIP -> DONE requires at least one canonical example passing nightly, benchmark artifact present, and linked migration-unblocked issue closed with reproducible steps.

## Detailed Week 1 (Qdrant) Plan

Day 1:
- Scaffold crate and workspace membership.
- Add Qdrant dependencies and builder/config/error/filter/mapper modules.
- Add crate `README.md`.

Day 2:
- Write failing contract tests first.
- Include collection-not-found test.

Day 3:
- Implement minimal passing `VectorStore` conformance.

Day 4:
- Add filter translation (`Eq`, `In`, `Range`, `All`, `Any`).
- Implement structured error mapping.
- Add one Layer-2 extension test (scored search pattern validation).

Day 5:
- Draft migration example.
- Integrate benchmark harness.
- Draft scoreboard row as WIP (do not mark DONE yet).

Week 1 end gate:
- Adapter compiles.
- Contract tests pass.
- Base trait behavior complete.
- Benchmark harness integrated.
- Scoreboard remains WIP.

## Weeks 3-4 Weaviate Template

Reuse the Qdrant pattern with Weaviate-specific deltas:
- GraphQL query/filter syntax translation with nested metadata path mapping.
- Class/schema behavior and optional auto-creation tests.
- Shared baseline dataset reuse when Qdrant nightly baseline is already valid.

End-of-slice gate remains merged + Green nightly with scoreboard DONE transition.

## Data and Reporting Locations

- Benchmark artifacts: `docs/benchmarks/data/`
- Waivers: `WAIVERS.yml`
- Migration readiness scoreboard: `docs/migration/scoreboard.md`

## Success Criteria at Week 12

- Targeted slices merged and validated.
- Nightly state Green (or justified Yellow under waiver policy).
- Scoreboard accurately reflects DONE status for completed slices.
- Reproducible migration-unblocked issues closed for completed integrations.
- Monthly release can ship from a stable, evidence-backed baseline.

## ADR Summary

ADR-001: Two-layer trait model.
- Context: need fast parity without repeated trait breakage.
- Decision: freeze Layer-1, add Layer-2 extensions and capability discovery.
- Consequence: stable migration surface plus additive differentiation path.

ADR-002: Structured error context.
- Context: production users need machine-actionable retry/alert behavior.
- Decision: crate-local typed errors mapped into structured core errors with source preservation.
- Consequence: predictable operational handling and easier debugging.

ADR-003: Benchmark governance split.
- Context: PR speed and release confidence require different gate strictness.
- Decision: PR advisory + nightly required thresholds.
- Consequence: fast iteration with defensible performance quality gates.
