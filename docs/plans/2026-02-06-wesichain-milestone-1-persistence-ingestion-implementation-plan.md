# Wesichain Milestone 1 Persistence and Ingestion Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement SQL checkpoint persistence (SQLite + Postgres wrappers, shared core, optional projections) and additive async ingestion parity (txt/pdf/docx + recursive splitter) without breaking existing APIs.

**Architecture:** Add a shared `wesichain-checkpoint-sql` crate for schema/migrations/errors/ops/projections and keep backend crates thin (`wesichain-checkpoint-sqlite`, `wesichain-checkpoint-postgres`). Keep canonical state in `checkpoints.state_json` and write projections transactionally when enabled. Extend `wesichain-retrieval` additively with async loaders and a new recursive splitter while preserving existing sync APIs.

**Tech Stack:** Rust 1.75, Tokio, sqlx, serde/serde_json, thiserror, docx-lite (or zip+xml fallback), criterion.

---

### Task 0: Baseline and scope lock

**Files:**
- Read: `docs/plans/2026-02-06-wesichain-milestone-1-persistence-ingestion-design.md`
- Read: `wesichain-graph/src/checkpoint.rs`
- Read: `wesichain-retrieval/src/loader.rs`
- Read: `wesichain-retrieval/src/splitter.rs`

**Step 1: Verify baseline tests in worktree**

Run: `cargo test --all -- --nocapture`
Expected: PASS with zero failures.

**Step 2: Record non-goals in notes**

Decision: No projection-based reconstruction, no JSONB migration, no breaking API changes in Milestone 1.

**Step 3: Commit**

No commit for this task.

---

### Task 1: Scaffold checkpoint SQL crates

**Files:**
- Modify: `Cargo.toml`
- Create: `wesichain-checkpoint-sql/Cargo.toml`
- Create: `wesichain-checkpoint-sql/src/lib.rs`
- Create: `wesichain-checkpoint-sql/src/error.rs`
- Create: `wesichain-checkpoint-sql/src/schema.rs`
- Create: `wesichain-checkpoint-sql/src/migrations.rs`
- Create: `wesichain-checkpoint-sql/src/dialect.rs`
- Create: `wesichain-checkpoint-sql/src/ops.rs`
- Create: `wesichain-checkpoint-sql/src/projection.rs`
- Create: `wesichain-checkpoint-sqlite/Cargo.toml`
- Create: `wesichain-checkpoint-sqlite/src/lib.rs`
- Create: `wesichain-checkpoint-postgres/Cargo.toml`
- Create: `wesichain-checkpoint-postgres/src/lib.rs`

**Step 1: Write failing compile check**

Run: `cargo check --workspace`
Expected: FAIL because new crates are not declared.

**Step 2: Add workspace members and crate manifests**

Add the three crates to workspace `members` and create minimal manifests with `sqlx`, `tokio`, `serde`, `serde_json`, `thiserror`, and required path dependencies.

**Step 3: Add minimal lib exports**

Create module stubs and public exports in each crate so compile succeeds before implementation.

**Step 4: Re-run compile check**

Run: `cargo check --workspace`
Expected: PASS.

**Step 5: Commit**

```bash
git add Cargo.toml wesichain-checkpoint-sql wesichain-checkpoint-sqlite wesichain-checkpoint-postgres
git commit -m "feat(checkpoint): scaffold sql checkpoint crates"
```

---

### Task 2: Add shared SQL schema and typed errors (TDD)

**Files:**
- Modify: `wesichain-checkpoint-sql/src/error.rs`
- Modify: `wesichain-checkpoint-sql/src/schema.rs`
- Create: `wesichain-checkpoint-sql/tests/schema.rs`

**Step 1: Write failing tests for schema constants and error displays**

Create `wesichain-checkpoint-sql/tests/schema.rs` with assertions that schema strings include `checkpoints`, `sessions`, `messages`, and `graph_triples`, and that typed errors format useful messages.

**Step 2: Run failing test**

Run: `cargo test -p wesichain-checkpoint-sql schema -v`
Expected: FAIL.

**Step 3: Implement `CheckpointSqlError` and DDL constants**

Implement explicit variants (`Connection`, `Migration`, `Serialization`, `Query`, `Projection`) and canonical/projection table SQL constants using `state_json TEXT NOT NULL`.

**Step 4: Run test again**

Run: `cargo test -p wesichain-checkpoint-sql schema -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-checkpoint-sql/src/error.rs wesichain-checkpoint-sql/src/schema.rs wesichain-checkpoint-sql/tests/schema.rs
git commit -m "feat(checkpoint): add shared schema and error types"
```

---

### Task 3: Implement canonical checkpoint ops (shared crate)

**Files:**
- Modify: `wesichain-checkpoint-sql/src/ops.rs`
- Modify: `wesichain-checkpoint-sql/src/migrations.rs`
- Create: `wesichain-checkpoint-sql/tests/ops_sqlite.rs`

**Step 1: Write failing SQLite ops tests**

Add tests for:
- migration bootstrap creates tables,
- save computes seq with `COALESCE(MAX(seq), 0) + 1` per thread,
- load returns latest row only.

**Step 2: Run failing tests**

Run: `cargo test -p wesichain-checkpoint-sql ops_sqlite -v`
Expected: FAIL.

**Step 3: Implement minimal shared ops**

Implement transactional save/load helper functions operating on sqlx transaction/executor and serializing checkpoint state via `serde_json`.

**Step 4: Run tests again**

Run: `cargo test -p wesichain-checkpoint-sql ops_sqlite -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-checkpoint-sql/src/ops.rs wesichain-checkpoint-sql/src/migrations.rs wesichain-checkpoint-sql/tests/ops_sqlite.rs
git commit -m "feat(checkpoint): implement canonical sql checkpoint ops"
```

---

### Task 4: Implement SQLite checkpointer backend

**Files:**
- Modify: `wesichain-checkpoint-sqlite/src/lib.rs`
- Create: `wesichain-checkpoint-sqlite/tests/checkpointer.rs`
- Modify: `wesichain-graph/Cargo.toml`
- Create: `wesichain-graph/tests/checkpointer_sqlite.rs`

**Step 1: Write failing backend tests**

Add tests for builder defaults (`enable_projections == false`) and `Checkpointer<S>` round-trip save/load behavior through graph integration.

**Step 2: Run failing tests**

Run: `cargo test -p wesichain-checkpoint-sqlite checkpointer -v`
Expected: FAIL.

**Step 3: Implement backend**

Add `SqliteCheckpointer` with builder, pool creation, migration bootstrap, and `Checkpointer<S>` implementation delegating to shared ops.

**Step 4: Run backend + graph integration tests**

Run: `cargo test -p wesichain-checkpoint-sqlite -v`
Run: `cargo test -p wesichain-graph checkpointer_sqlite -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-checkpoint-sqlite wesichain-graph/Cargo.toml wesichain-graph/tests/checkpointer_sqlite.rs
git commit -m "feat(checkpoint-sqlite): add sqlite checkpointer backend"
```

---

### Task 5: Add optional projections and rollback behavior

**Files:**
- Modify: `wesichain-checkpoint-sql/src/projection.rs`
- Modify: `wesichain-checkpoint-sql/src/ops.rs`
- Modify: `wesichain-checkpoint-sqlite/src/lib.rs`
- Create: `wesichain-checkpoint-sql/tests/projection.rs`

**Step 1: Write failing projection tests**

Add tests for:
- projections disabled: only `checkpoints` row is written,
- projections enabled: `sessions/messages` rows are written,
- forced projection error rolls back canonical checkpoint insert.

**Step 2: Run failing tests**

Run: `cargo test -p wesichain-checkpoint-sql projection -v`
Expected: FAIL.

**Step 3: Implement projection mapper and tx wiring**

Implement state-to-row mapping (sessions/messages minimal parity, triples from state when available or no-op) and transaction rollback on projection error.

**Step 4: Run tests again**

Run: `cargo test -p wesichain-checkpoint-sql projection -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-checkpoint-sql/src/projection.rs wesichain-checkpoint-sql/src/ops.rs wesichain-checkpoint-sqlite/src/lib.rs wesichain-checkpoint-sql/tests/projection.rs
git commit -m "feat(checkpoint): add optional projections and rollback semantics"
```

---

### Task 6: Implement Postgres backend parity

**Files:**
- Modify: `wesichain-checkpoint-postgres/src/lib.rs`
- Create: `wesichain-checkpoint-postgres/tests/checkpointer.rs`

**Step 1: Write failing backend compile/test scaffold**

Add tests gated behind an env var/feature that verify builder construction and round-trip semantics when Postgres is available.

**Step 2: Run failing tests (or skipped when env missing)**

Run: `cargo test -p wesichain-checkpoint-postgres -v`
Expected: FAIL before implementation (or compile errors).

**Step 3: Implement backend wrapper**

Implement thin Postgres backend: pool options, migrations on init, shared ops usage, projection flag support.

**Step 4: Re-run tests**

Run: `cargo test -p wesichain-checkpoint-postgres -v`
Expected: PASS (with integration test skip if no `DATABASE_URL`).

**Step 5: Commit**

```bash
git add wesichain-checkpoint-postgres
git commit -m "feat(checkpoint-postgres): add postgres checkpointer backend"
```

---

### Task 7: Add additive async ingestion API surface

**Files:**
- Modify: `wesichain-retrieval/src/lib.rs`
- Modify: `wesichain-retrieval/src/loader.rs`
- Modify: `wesichain-retrieval/src/error.rs`
- Modify: `wesichain-retrieval/Cargo.toml`
- Create: `wesichain-retrieval/tests/async_loader.rs`

**Step 1: Write failing tests for async loader dispatch**

Test `.txt` async read success and unsupported extension error path while preserving existing sync loader behavior.

**Step 2: Run failing tests**

Run: `cargo test -p wesichain-retrieval async_loader -v`
Expected: FAIL.

**Step 3: Implement additive async APIs**

Add `load_file_async`, `load_files_async`, and stage-specific `IngestionError` variants with path context. Keep existing sync APIs and exports intact.

**Step 4: Run tests again**

Run: `cargo test -p wesichain-retrieval async_loader -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-retrieval/src/lib.rs wesichain-retrieval/src/loader.rs wesichain-retrieval/src/error.rs wesichain-retrieval/Cargo.toml wesichain-retrieval/tests/async_loader.rs
git commit -m "feat(retrieval): add additive async ingestion api"
```

---

### Task 8: Implement DOCX text-first extraction

**Files:**
- Modify: `wesichain-retrieval/src/loader.rs`
- Create: `wesichain-retrieval/tests/docx_loader.rs`
- Add fixture(s): `wesichain-retrieval/tests/fixtures/*.docx`

**Step 1: Write failing DOCX tests**

Test paragraph and table-cell flattening into plain text with deterministic separators and metadata.

**Step 2: Run failing tests**

Run: `cargo test -p wesichain-retrieval docx_loader -v`
Expected: FAIL.

**Step 3: Implement DOCX adapter**

Implement text-first extraction using `docx-lite` (or zip+xml fallback) with graceful parse errors and no rich layout semantics.

**Step 4: Run tests again**

Run: `cargo test -p wesichain-retrieval docx_loader -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-retrieval/src/loader.rs wesichain-retrieval/tests/docx_loader.rs wesichain-retrieval/tests/fixtures
git commit -m "feat(retrieval): add docx text-first loader"
```

---

### Task 9: Add recursive character splitter (new type)

**Files:**
- Modify: `wesichain-retrieval/src/splitter.rs`
- Modify: `wesichain-retrieval/src/lib.rs`
- Create: `wesichain-retrieval/tests/recursive_splitter.rs`

**Step 1: Write failing splitter tests**

Cover:
- separator priority behavior,
- UTF-8-safe boundaries,
- overlap windows,
- `chunk_size == 0` config error,
- overlap clamp behavior,
- metadata propagation (`chunk_index`, `source`).

**Step 2: Run failing tests**

Run: `cargo test -p wesichain-retrieval recursive_splitter -v`
Expected: FAIL.

**Step 3: Implement `RecursiveCharacterTextSplitter`**

Add builder-style constructor and recursive split algorithm with defaults `["\n\n", "\n", " ", ""]`.

**Step 4: Run tests again**

Run: `cargo test -p wesichain-retrieval recursive_splitter -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-retrieval/src/splitter.rs wesichain-retrieval/src/lib.rs wesichain-retrieval/tests/recursive_splitter.rs
git commit -m "feat(retrieval): add recursive character splitter"
```

---

### Task 10: Add `load_and_split_recursive` and integration tests

**Files:**
- Modify: `wesichain-retrieval/src/lib.rs`
- Create: `wesichain-retrieval/tests/ingestion_integration.rs`

**Step 1: Write failing integration test**

Test end-to-end load (txt/docx fixture) -> recursive split -> document chunk metadata expectations.

**Step 2: Run failing test**

Run: `cargo test -p wesichain-retrieval ingestion_integration -v`
Expected: FAIL.

**Step 3: Implement convenience API**

Add `load_and_split_recursive` helper that composes new async load APIs and new splitter.

**Step 4: Run test again**

Run: `cargo test -p wesichain-retrieval ingestion_integration -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-retrieval/src/lib.rs wesichain-retrieval/tests/ingestion_integration.rs
git commit -m "feat(retrieval): add async load-and-split convenience"
```

---

### Task 11: Benchmark and documentation polish

**Files:**
- Create: `wesichain-retrieval/benches/recursive_splitter.rs`
- Create/Modify: `wesichain-retrieval/examples/*.rs` (as needed)
- Modify: `wesichain-retrieval/Cargo.toml`
- Modify: `docs/plans/2026-02-06-wesichain-milestone-1-persistence-ingestion-design.md` (only if clarifications needed)

**Step 1: Add failing benchmark compile check**

Run: `cargo bench -p wesichain-retrieval --bench recursive_splitter --no-run`
Expected: FAIL before bench target exists.

**Step 2: Implement criterion benchmark target**

Add throughput benchmark using representative text sizes and report-friendly labels.

**Step 3: Verify benchmark target compiles**

Run: `cargo bench -p wesichain-retrieval --bench recursive_splitter --no-run`
Expected: PASS.

**Step 4: Run focused crate checks**

Run: `cargo test -p wesichain-checkpoint-sql -v`
Run: `cargo test -p wesichain-checkpoint-sqlite -v`
Run: `cargo test -p wesichain-checkpoint-postgres -v`
Run: `cargo test -p wesichain-retrieval -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-retrieval/benches/recursive_splitter.rs wesichain-retrieval/Cargo.toml
git commit -m "bench(retrieval): add recursive splitter benchmark"
```

---

### Task 12: Final verification and release-ready check

**Files:**
- Modify: none

**Step 1: Format and lint**

Run: `cargo fmt --all`
Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS.

**Step 2: Full workspace tests**

Run: `cargo test --all -- --nocapture`
Expected: PASS.

**Step 3: Verify git status**

Run: `git status --short`
Expected: clean working tree.

**Step 4: Commit**

No commit in this task unless format/lint introduced changes.

---

## Validation Checklist
- [ ] `Checkpointer<S>` integration remains source-compatible.
- [ ] SQLite backend supports canonical checkpoint save/load.
- [ ] Projection flag defaults to off and is transactional when on.
- [ ] Projection rollback leaves no canonical row on failure.
- [ ] Postgres backend compiles and passes parity tests when configured.
- [ ] Existing retrieval sync APIs remain unchanged.
- [ ] Async loaders support txt/pdf/docx with explicit errors.
- [ ] Recursive splitter is UTF-8 safe and deterministic.
- [ ] Bench target compiles and produces reproducible numbers.

## Suggested Verification Commands
- `cargo check --workspace`
- `cargo test -p wesichain-checkpoint-sql -v`
- `cargo test -p wesichain-checkpoint-sqlite -v`
- `cargo test -p wesichain-checkpoint-postgres -v`
- `cargo test -p wesichain-retrieval -v`
- `cargo test --all -- --nocapture`
