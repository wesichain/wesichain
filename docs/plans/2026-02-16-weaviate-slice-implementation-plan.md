# Wesichain Weaviate Migration Slice Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement and ship `wesichain-weaviate` as a migration-unblocking vector store slice with tests, migration artifacts, benchmarks, and CI gates.

**Architecture:** Add a dedicated `wesichain-weaviate` crate that implements `wesichain_core::VectorStore` without changing frozen Layer-1 core contracts. Keep Weaviate-specific GraphQL/class schema concerns local to the crate, validate parity through contract + migration tests, and finish with benchmark/governance artifacts linked in migration docs.

**Tech Stack:** Rust workspace crates, `reqwest`, `serde_json`, `thiserror`, `tokio`, GitHub Actions, Criterion benchmarks.

---

Implementation skills to apply while executing this plan:
- @test-driven-development
- @verification-before-completion
- @systematic-debugging

### Task 1: Scaffold `wesichain-weaviate` crate and builder defaults

**Files:**
- Modify: `Cargo.toml`
- Create: `wesichain-weaviate/Cargo.toml`
- Create: `wesichain-weaviate/src/lib.rs`
- Create: `wesichain-weaviate/src/config.rs`
- Create: `wesichain-weaviate/src/error.rs`
- Create: `wesichain-weaviate/src/filter.rs`
- Create: `wesichain-weaviate/src/mapper.rs`
- Create: `wesichain-weaviate/README.md`
- Test: `wesichain-weaviate/tests/builder.rs`

**Step 1: Write failing builder test**

```rust
#[tokio::test]
async fn builder_allows_optional_api_key_and_requires_class() {
    let result = wesichain_weaviate::WeaviateVectorStore::builder()
        .base_url("http://localhost:8080")
        .build()
        .await;
    assert!(result.is_err());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p wesichain-weaviate builder_allows_optional_api_key_and_requires_class -- --nocapture`
Expected: FAIL because crate/modules do not exist yet.

**Step 3: Write minimal implementation**

```rust
pub struct WeaviateStoreBuilder {
    base_url: Option<String>,
    class_name: Option<String>,
    api_key: Option<String>,
}
```

Implement optional API key + cloud URL warning when missing key.

**Step 4: Run test to verify it passes**

Run: `cargo test -p wesichain-weaviate builder_allows_optional_api_key_and_requires_class -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add Cargo.toml wesichain-weaviate
git commit -m "feat(weaviate): scaffold crate and optional-auth builder"
```

### Task 2: Add contract tests and basic add/search/delete implementation

**Files:**
- Create: `wesichain-weaviate/tests/contract_tests.rs`
- Modify: `wesichain-weaviate/src/lib.rs`
- Modify: `wesichain-weaviate/src/mapper.rs`

**Step 1: Write failing contract tests**

```rust
#[tokio::test]
async fn contract_add_search_delete_roundtrip() {}

#[tokio::test]
async fn contract_class_not_found_returns_clear_error() {}

#[tokio::test]
async fn contract_class_auto_creation_bootstraps_schema() {}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-weaviate contract_ -- --nocapture`
Expected: FAIL due missing implementation.

**Step 3: Implement minimal behavior**

```rust
#[async_trait::async_trait]
impl VectorStore for WeaviateVectorStore {
    async fn add(&self, docs: Vec<Document>) -> Result<(), StoreError> { /* ... */ }
    async fn search(&self, query_embedding: &[f32], top_k: usize, filter: Option<&MetadataFilter>) -> Result<Vec<SearchResult>, StoreError> { /* ... */ }
    async fn delete(&self, ids: &[String]) -> Result<(), StoreError> { /* ... */ }
}
```

Use `RUN_WEAVIATE_CONTRACT=1` gate for live integration tests.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-weaviate contract_ -- --nocapture`
Expected: PASS or gated-skip when env var unset.

**Step 5: Commit**

```bash
git add wesichain-weaviate/src/lib.rs wesichain-weaviate/src/mapper.rs wesichain-weaviate/tests/contract_tests.rs
git commit -m "feat(weaviate): implement vectorstore contract roundtrip"
```

### Task 3: Implement Weaviate GraphQL metadata filter translation

**Files:**
- Modify: `wesichain-weaviate/src/filter.rs`
- Modify: `wesichain-weaviate/src/lib.rs`
- Test: `wesichain-weaviate/tests/filter.rs`

**Step 1: Write failing filter tests**

```rust
#[test]
fn translates_eq_in_range_all_any_to_weaviate_where_clause() {}

#[test]
fn translates_nested_path_to_metadata_path_segments() {}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-weaviate filter -- --nocapture`
Expected: FAIL due missing translation.

**Step 3: Implement minimal translator**

```rust
pub fn to_weaviate_where(filter: &MetadataFilter) -> Result<serde_json::Value, WeaviateStoreError> {
    // GraphQL `where` translation with nested path support
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-weaviate filter -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-weaviate/src/filter.rs wesichain-weaviate/src/lib.rs wesichain-weaviate/tests/filter.rs
git commit -m "feat(weaviate): add graphql where filter translation"
```

### Task 4: Add structured errors and scored-search validation

**Files:**
- Modify: `wesichain-weaviate/src/error.rs`
- Modify: `wesichain-weaviate/src/lib.rs`
- Test: `wesichain-weaviate/tests/error.rs`
- Test: `wesichain-weaviate/tests/scored_search.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn weaviate_error_maps_with_backend_and_operation_context() {}

#[tokio::test]
async fn scored_search_returns_descending_scores() {}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p wesichain-weaviate error scored_search -- --nocapture`
Expected: FAIL due missing mappings/behavior.

**Step 3: Implement minimal behavior**

```rust
#[derive(Debug, thiserror::Error)]
pub enum WeaviateStoreError {
    #[error("schema/class not found: {class}")]
    ClassNotFound { class: String },
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p wesichain-weaviate -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-weaviate/src/error.rs wesichain-weaviate/src/lib.rs wesichain-weaviate/tests/error.rs wesichain-weaviate/tests/scored_search.rs
git commit -m "fix(weaviate): add structured error mapping and scored search validation"
```

### Task 5: Add migration artifacts and scoreboard WIP row

**Files:**
- Create: `wesichain-weaviate/examples/rag_integration.rs`
- Create: `wesichain-weaviate/tests/migration_parity.rs`
- Create: `docs/migration/langchain-to-wesichain-weaviate.md`
- Modify: `docs/migration/scoreboard.md`

**Step 1: Write failing migration parity test**

```rust
#[tokio::test]
async fn migration_parity_core_flow_matches_expected_behavior() {}
```

**Step 2: Run test to verify failure**

Run: `cargo test -p wesichain-weaviate migration_parity -- --nocapture`
Expected: FAIL because files/example missing.

**Step 3: Implement migration guide + example + parity test**

```rust
// examples/rag_integration.rs: ingest -> query -> delete
```

**Step 4: Run checks**

Run: `cargo test -p wesichain-weaviate --tests -- --nocapture && cargo check -p wesichain-weaviate --examples`
Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-weaviate/examples/rag_integration.rs wesichain-weaviate/tests/migration_parity.rs docs/migration/langchain-to-wesichain-weaviate.md docs/migration/scoreboard.md
git commit -m "docs(weaviate): add migration guide and parity artifacts"
```

### Task 6: Add Weaviate benchmark harness and machine-readable artifact

**Files:**
- Create: `wesichain-weaviate/benches/vs_langchain.rs`
- Create/Modify: `docs/benchmarks/data/weaviate-2026-02-16.json`
- Modify: `tools/bench/README.md`

**Step 1: Write failing benchmark metadata test**

```rust
#[test]
fn benchmark_metadata_includes_dataset_commit_hash() {}
```

**Step 2: Run check to verify it fails**

Run: `cargo test -p wesichain-weaviate benchmark_metadata_includes_dataset_commit_hash -- --nocapture`
Expected: FAIL before harness exists.

**Step 3: Implement benchmark harness**

```rust
const DATASET_COMMIT: &str = "<pin-me>";
```

**Step 4: Run benchmark**

Run: `cargo bench -p wesichain-weaviate --bench vs_langchain -- --sample-size 10`
Expected: runs and emits criterion output.

**Step 5: Commit**

```bash
git add wesichain-weaviate/benches/vs_langchain.rs docs/benchmarks/data/weaviate-2026-02-16.json tools/bench/README.md
git commit -m "test(weaviate): add benchmark harness and artifact"
```

### Task 7: Ensure CI and impact-map coverage includes Weaviate

**Files:**
- Modify: `.github/workflows/pr-checks.yml`
- Modify: `.github/workflows/nightly-bench.yml`
- Modify: `tools/ci/impact-map.toml`
- Modify: `wesichain/tests/ci_config_validation.rs`

**Step 1: Write failing CI coverage assertion**

```rust
#[test]
fn ci_impact_map_includes_weaviate_connector_example() {}
```

**Step 2: Run test to verify failure**

Run: `cargo test -p wesichain --test ci_config_validation -- --nocapture`
Expected: FAIL before CI config updates.

**Step 3: Implement updates**

- include `wesichain-weaviate` in connector fan-out group.
- include weaviate benchmark path in nightly where applicable.

**Step 4: Run test to verify pass**

Run: `cargo test -p wesichain --test ci_config_validation -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add .github/workflows/pr-checks.yml .github/workflows/nightly-bench.yml tools/ci/impact-map.toml wesichain/tests/ci_config_validation.rs
git commit -m "ci(weaviate): include weaviate in impact and nightly coverage"
```

### Task 8: Close slice readiness and publish `wesichain-weaviate`

**Files:**
- Modify: `docs/migration/scoreboard.md`
- Modify: `docs/benchmarks/data/weekly/2026-W08.md`
- Modify: `CHANGELOG.md`
- Modify: `README.md`

**Step 1: Run readiness check and verify initial failure**

Run: `python3 tools/bench/check_release_readiness.py --slice weaviate`
Expected: FAIL until placeholders/evidence are complete.

**Step 2: Complete readiness evidence**

- trigger and record nightly run URL.
- create/close migration-unblocked issue for Weaviate.
- update scoreboard WIP -> DONE with links.

**Step 3: Publish validation + crate publish**

Run:

```bash
cargo fmt --all -- --check
cargo clippy -p wesichain-weaviate --all-targets -- -D warnings
cargo test -p wesichain-weaviate --tests -- --nocapture
cargo check -p wesichain-weaviate --examples
cargo publish -p wesichain-weaviate --dry-run
cargo publish -p wesichain-weaviate
```

Expected: all commands pass and crate is published.

**Step 4: Re-run readiness check**

Run: `python3 tools/bench/check_release_readiness.py --slice weaviate`
Expected: PASS.

**Step 5: Commit**

```bash
git add docs/migration/scoreboard.md docs/benchmarks/data/weekly/2026-W08.md CHANGELOG.md README.md
git commit -m "docs(weaviate): close migration slice readiness evidence"
```

## Verification Bundle Before Merge

Run all:

```bash
cargo fmt --all -- --check
cargo clippy -p wesichain-weaviate --all-targets -- -D warnings
cargo test -p wesichain-weaviate --tests -- --nocapture
cargo test -p wesichain --test ci_config_validation -- --nocapture
cargo check -p wesichain-weaviate --examples
python3 tools/bench/check_release_readiness.py --slice weaviate
```

Expected: all pass at closeout.
