# Wesichain Qdrant Migration Slice Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a production-ready `wesichain-qdrant` vector store slice that unblocks LangChain-style migrations with full tests, migration example, and benchmark artifacts.

**Architecture:** Add a dedicated `wesichain-qdrant` crate that implements `wesichain_core::VectorStore` without changing existing Layer-1 core contracts. Keep Qdrant-specific concerns (builder, filter translation, backend errors) inside the crate, then prove migration readiness via contract tests, example parity, and benchmark reporting. Ship in two weeks, ending at `merged + nightly_green` (no release tag).

**Tech Stack:** Rust workspace crates, `qdrant-client`, `tokio`, `async-trait`, `thiserror`, `serde_json`, `tracing`, GitHub Actions, Criterion benchmarks.

---

Implementation skills to apply while executing this plan:
- @test-driven-development
- @verification-before-completion
- @systematic-debugging

### Task 1: Scaffold `wesichain-qdrant` crate and builder defaults

**Files:**
- Modify: `Cargo.toml`
- Create: `wesichain-qdrant/Cargo.toml`
- Create: `wesichain-qdrant/src/lib.rs`
- Create: `wesichain-qdrant/src/config.rs`
- Create: `wesichain-qdrant/src/error.rs`
- Create: `wesichain-qdrant/src/filter.rs`
- Create: `wesichain-qdrant/src/mapper.rs`
- Create: `wesichain-qdrant/README.md`
- Test: `wesichain-qdrant/tests/builder.rs`

**Step 1: Write the failing builder test**

```rust
use wesichain_qdrant::QdrantVectorStore;

#[tokio::test]
async fn builder_allows_optional_api_key_and_requires_collection() {
    let result = QdrantVectorStore::builder()
        .base_url("http://localhost:6333")
        .build()
        .await;

    assert!(result.is_err());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-qdrant builder_allows_optional_api_key_and_requires_collection -- --nocapture`
Expected: FAIL because crate/modules do not exist yet.

**Step 3: Write minimal implementation**

```rust
pub struct QdrantStoreBuilder {
    base_url: Option<String>,
    collection: Option<String>,
    api_key: Option<String>,
}

impl QdrantStoreBuilder {
    pub fn api_key(mut self, value: impl Into<String>) -> Self {
        self.api_key = Some(value.into());
        self
    }
}
```

Also:
- Add workspace member entry in root `Cargo.toml`.
- Add optional API-key behavior and cloud URL warning in builder.

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-qdrant builder_allows_optional_api_key_and_requires_collection -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add Cargo.toml wesichain-qdrant
git commit -m "feat(qdrant): scaffold crate and optional-auth builder"
```

### Task 2: Add base contract tests for add/search/delete

**Files:**
- Create: `wesichain-qdrant/tests/contract_tests.rs`
- Modify: `wesichain-qdrant/src/lib.rs`
- Modify: `wesichain-qdrant/src/mapper.rs`

**Step 1: Write failing contract tests**

```rust
#[tokio::test]
async fn contract_add_search_delete_roundtrip() {
    // Use RUN_QDRANT_CONTRACT=1 gate before connecting to Qdrant.
    // Add docs with embeddings, search top_k, delete ids, verify empty.
}

#[tokio::test]
async fn contract_collection_not_found_is_clear_error() {
    // Build store with missing collection and assert typed error mapping.
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-qdrant contract_ -- --nocapture`
Expected: FAIL due unimplemented `VectorStore` behavior.

**Step 3: Implement minimal `VectorStore` support**

```rust
#[async_trait::async_trait]
impl VectorStore for QdrantVectorStore {
    async fn add(&self, docs: Vec<Document>) -> Result<(), StoreError> { /* ... */ }
    async fn search(&self, query_embedding: &[f32], top_k: usize, filter: Option<&MetadataFilter>)
        -> Result<Vec<SearchResult>, StoreError> { /* ... */ }
    async fn delete(&self, ids: &[String]) -> Result<(), StoreError> { /* ... */ }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-qdrant contract_ -- --nocapture`
Expected: PASS (or skipped when `RUN_QDRANT_CONTRACT` is unset).

**Step 5: Commit**

```bash
git add wesichain-qdrant/src/lib.rs wesichain-qdrant/src/mapper.rs wesichain-qdrant/tests/contract_tests.rs
git commit -m "feat(qdrant): implement vectorstore contract roundtrip"
```

### Task 3: Implement metadata filter translation and Weaviate-parity style coverage

**Files:**
- Modify: `wesichain-qdrant/src/filter.rs`
- Modify: `wesichain-qdrant/src/lib.rs`
- Test: `wesichain-qdrant/tests/filter.rs`

**Step 1: Write failing filter translation tests**

```rust
#[test]
fn translates_eq_in_range_all_any_filters() {
    // MetadataFilter::Eq/In/Range/All/Any => Qdrant filter payload.
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-qdrant translates_eq_in_range_all_any_filters -- --nocapture`
Expected: FAIL due missing translator.

**Step 3: Implement translation layer**

```rust
pub fn to_qdrant_filter(filter: &MetadataFilter) -> Result<qdrant_client::qdrant::Filter, QdrantStoreError> {
    // map Eq/In/Range/All/Any recursively
}
```

Include nested metadata key support and deterministic mapping.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-qdrant filter -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-qdrant/src/filter.rs wesichain-qdrant/src/lib.rs wesichain-qdrant/tests/filter.rs
git commit -m "feat(qdrant): add metadata filter translation"
```

### Task 4: Add structured error mapping and extension-pattern validation

**Files:**
- Modify: `wesichain-qdrant/src/error.rs`
- Modify: `wesichain-qdrant/src/lib.rs`
- Test: `wesichain-qdrant/tests/error.rs`
- Test: `wesichain-qdrant/tests/scored_search.rs`

**Step 1: Write failing error and scored-search tests**

```rust
#[test]
fn qdrant_error_maps_with_backend_and_operation_context() {
    // assert mapping into StoreError::Internal preserves source.
}

#[tokio::test]
async fn scored_search_returns_descending_scores() {
    // extension-pattern test for scored retrieval path.
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-qdrant error scored_search -- --nocapture`
Expected: FAIL due missing mapping/method behavior.

**Step 3: Implement minimal behavior**

```rust
#[derive(Debug, thiserror::Error)]
pub enum QdrantStoreError {
    #[error("collection not found: {collection}")]
    CollectionNotFound { collection: String },
    #[error("qdrant api error: {0}")]
    Api(String),
}
```

Add a scored-search helper path that keeps results sorted descending.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-qdrant -- --nocapture`
Expected: PASS for new tests.

**Step 5: Commit**

```bash
git add wesichain-qdrant/src/error.rs wesichain-qdrant/src/lib.rs wesichain-qdrant/tests/error.rs wesichain-qdrant/tests/scored_search.rs
git commit -m "fix(qdrant): map backend errors and validate scored search"
```

### Task 5: Add migration example and docs (Week 1 WIP state)

**Files:**
- Create: `wesichain-qdrant/examples/rag_integration.rs`
- Create: `docs/migration/langchain-to-wesichain-qdrant.md`
- Modify: `docs/migration/scoreboard.md`

**Step 1: Write failing migration parity test**

```rust
#[tokio::test]
async fn qdrant_example_compiles_and_runs_core_flow() {
    // ingest -> query -> delete using crate-local example helpers
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-qdrant qdrant_example_compiles_and_runs_core_flow -- --nocapture`
Expected: FAIL because example/parity harness is missing.

**Step 3: Implement migration example + guide**

```rust
// examples/rag_integration.rs
// 1) build embedder
// 2) build QdrantVectorStore
// 3) add docs with embeddings
// 4) query by embedding
```

Include side-by-side LangChain parity snippets in doc.

**Step 4: Run tests/compile checks**

Run: `cargo test -p wesichain-qdrant --tests -- --nocapture && cargo check -p wesichain-qdrant --examples`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-qdrant/examples/rag_integration.rs docs/migration/langchain-to-wesichain-qdrant.md docs/migration/scoreboard.md
git commit -m "docs(qdrant): add migration example and mark scoreboard wip"
```

### Task 6: Integrate benchmark harness and machine-readable output

**Files:**
- Create: `wesichain-qdrant/benches/vs_langchain.rs`
- Modify: `tools/bench/README.md`
- Create/Modify: `docs/benchmarks/data/qdrant-<date>.json`

**Step 1: Write failing benchmark smoke test**

```rust
#[test]
fn benchmark_metadata_includes_dataset_commit_hash() {
    // parse json output and assert commit hash is present
}
```

**Step 2: Run benchmark check to verify failure**

Run: `cargo test -p wesichain-qdrant benchmark_metadata_includes_dataset_commit_hash -- --nocapture`
Expected: FAIL due missing harness/output metadata.

**Step 3: Implement minimal benchmark harness**

```rust
const DATASET_COMMIT: &str = "<pin-me>";
// write bench output json with dataset + commit + hardware fields
```

**Step 4: Run benchmark smoke validation**

Run: `cargo bench -p wesichain-qdrant --bench vs_langchain -- --sample-size 10`
Expected: benchmark runs and emits machine-readable output.

**Step 5: Commit**

```bash
git add wesichain-qdrant/benches/vs_langchain.rs tools/bench/README.md docs/benchmarks/data
git commit -m "test(qdrant): add benchmark harness and reproducible metadata"
```

### Task 7: Wire PR advisory and nightly-required checks for Qdrant slice

**Files:**
- Modify: `.github/workflows/pr-checks.yml`
- Modify: `.github/workflows/nightly-bench.yml`
- Create/Modify: `tools/bench/thresholds.toml`
- Create/Modify: `WAIVERS.yml`
- Create/Modify: `tools/ci/impact-map.toml`

**Step 1: Write failing CI config-level tests/checks**

```bash
# Add a dry-run script assertion for required keys and paths.
```

**Step 2: Run CI dry-run checks to verify failure**

Run: `cargo test -p wesichain --test ci_config_validation -- --nocapture`
Expected: FAIL before workflow updates.

**Step 3: Implement workflow and threshold updates**

Required policy:
- PR: touched-crate compile/tests + advisory benchmark.
- Nightly: required benchmark thresholds (p50/p95/p99/index/memory/error).
- Core trait changes: fan out compile to all connector examples.

**Step 4: Re-run CI config validation locally**

Run: `cargo test -p wesichain --test ci_config_validation -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add .github/workflows tools/bench/thresholds.toml WAIVERS.yml tools/ci/impact-map.toml
git commit -m "ci(qdrant): add advisory pr and required nightly benchmark gates"
```

### Task 8: Week 2 closure gate (`merged + nightly_green`)

**Files:**
- Modify: `docs/migration/scoreboard.md`
- Modify: `docs/benchmarks/data/weekly/<week>.md`
- Modify: `docs/migration/langchain-to-wesichain-qdrant.md`

**Step 1: Write failing release-readiness check**

```bash
# Add script/assertion requiring: example nightly pass + benchmark artifact + closed unblock issue.
```

**Step 2: Run readiness check and verify failure**

Run: `python tools/bench/check_release_readiness.py --slice qdrant`
Expected: FAIL before nightly-green and scoreboard DONE state.

**Step 3: Complete readiness inputs**

- Trigger nightly dry run.
- Confirm state is Green (or approved Yellow).
- Update scoreboard row `Qdrant` from WIP to DONE.
- Add reproducible commands and issue link.

**Step 4: Re-run readiness check**

Run: `python tools/bench/check_release_readiness.py --slice qdrant`
Expected: PASS.

**Step 5: Commit**

```bash
git add docs/migration/scoreboard.md docs/benchmarks/data docs/migration/langchain-to-wesichain-qdrant.md
git commit -m "docs(qdrant): close migration slice at nightly-green"
```

## Verification Bundle Before Marking Done

Run all:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test -p wesichain-qdrant -- --nocapture
cargo check -p wesichain-qdrant --examples
```

Expected:
- All commands pass.
- Qdrant contract tests pass (or skip only when explicit runtime gates are unset).
- Migration example compiles.
- Benchmark artifact exists with pinned metadata.

## Notes

- Do not cut a release tag in Week 2.
- Monthly release batching remains anchored to last Thursday UTC.
- Keep Week 1 scoreboard state as WIP until Week 2 closure criteria are met.
