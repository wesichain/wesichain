# Wesichain Pinecone Vector Store Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a new `wesichain-pinecone` crate that provides LangChain-compatible Pinecone vector store ergonomics with Rust-idiomatic APIs, external embeddings, and data-plane-only operations.

**Architecture:** Implement a standalone integration crate with layered modules: builder/config, typed HTTP client, request/response mappers, and `PineconeVectorStore<E: Embedding>` orchestration. Use `reqwest + serde` for Pinecone REST calls and implement `VectorStore` trait plus LangChain-style wrappers (`add_documents`, `similarity_search`, `similarity_search_with_score`, `delete`). Keep scope strict to data-plane operations and add optional warning-only dimension validation.

**Tech Stack:** Rust 1.75, tokio, async-trait, reqwest, serde/serde_json, thiserror, tracing, uuid, wiremock, wesichain-core.

---

### Task 0: Baseline and workspace prep

**Files:**
- Read: `docs/plans/2026-02-06-wesichain-pinecone-vector-store-design.md`
- Modify: `Cargo.toml`

**Step 1: Verify clean baseline in worktree**

Run: `cargo test`

Expected: PASS for entire workspace.

**Step 2: Add new workspace member**

Update `Cargo.toml` workspace members:

```toml
members = [
  "wesichain",
  "wesichain-core",
  "wesichain-prompt",
  "wesichain-llm",
  "wesichain-agent",
  "wesichain-graph",
  "wesichain-embeddings",
  "wesichain-retrieval",
  "wesichain-langsmith",
  "wesichain-pinecone",
]
```

**Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore(workspace): add wesichain-pinecone member"
```

---

### Task 1: Scaffold `wesichain-pinecone` crate

**Files:**
- Create: `wesichain-pinecone/Cargo.toml`
- Create: `wesichain-pinecone/src/lib.rs`
- Create: `wesichain-pinecone/src/error.rs`
- Create: `wesichain-pinecone/src/config.rs`
- Create: `wesichain-pinecone/src/types.rs`
- Create: `wesichain-pinecone/src/filter.rs`
- Create: `wesichain-pinecone/src/mapper.rs`
- Create: `wesichain-pinecone/src/client.rs`
- Create: `wesichain-pinecone/src/store.rs`

**Step 1: Write failing compile target**

Create `wesichain-pinecone/src/lib.rs` with module declarations and re-exports (modules not implemented yet):

```rust
mod client;
mod config;
mod error;
mod filter;
mod mapper;
mod store;
mod types;

pub use config::PineconeStoreBuilder;
pub use error::PineconeStoreError;
pub use store::PineconeVectorStore;
```

**Step 2: Run compile to verify failure**

Run: `cargo test -p wesichain-pinecone -v`

Expected: FAIL (crate missing files/manifest).

**Step 3: Add minimal manifest and module stubs**

Create `wesichain-pinecone/Cargo.toml`:

```toml
[package]
name = "wesichain-pinecone"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
repository.workspace = true
homepage.workspace = true
description = "Pinecone vector store integration for Wesichain"

[dependencies]
async-trait = "0.1"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
tracing = "0.1"
uuid = { version = "1", features = ["v4"] }
wesichain-core = { path = "../wesichain-core" }

[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
wiremock = "0.6"
```

Create empty stubs for remaining modules with `pub struct Placeholder;`/`pub enum Placeholder {}` so crate compiles.

**Step 4: Run compile to verify pass**

Run: `cargo test -p wesichain-pinecone -v`

Expected: PASS (0 tests or stub tests).

**Step 5: Commit**

```bash
git add wesichain-pinecone/Cargo.toml wesichain-pinecone/src/lib.rs wesichain-pinecone/src/error.rs wesichain-pinecone/src/config.rs wesichain-pinecone/src/types.rs wesichain-pinecone/src/filter.rs wesichain-pinecone/src/mapper.rs wesichain-pinecone/src/client.rs wesichain-pinecone/src/store.rs
git commit -m "feat(pinecone): scaffold integration crate"
```

---

### Task 2: Define error model and conversions

**Files:**
- Modify: `wesichain-pinecone/src/error.rs`
- Modify: `wesichain-pinecone/src/lib.rs`
- Test: `wesichain-pinecone/tests/error.rs`

**Step 1: Write failing tests**

Create `wesichain-pinecone/tests/error.rs`:

```rust
use wesichain_core::StoreError;
use wesichain_pinecone::PineconeStoreError;

#[test]
fn pinecone_error_converts_to_store_error() {
    let err = PineconeStoreError::Config("missing api key".to_string());
    let store_err: StoreError = err.into();
    assert!(format!("{store_err}").contains("Store error"));
}

#[test]
fn api_error_includes_status_and_message() {
    let err = PineconeStoreError::Api {
        status: 429,
        message: "rate limited".to_string(),
        retry_after_seconds: Some(30),
        namespace: Some("prod".to_string()),
        batch_size: Some(50),
    };
    let text = err.to_string();
    assert!(text.contains("429"));
    assert!(text.contains("rate limited"));
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-pinecone error -v`

Expected: FAIL (error variants/conversion missing).

**Step 3: Implement error enum and conversion**

Update `wesichain-pinecone/src/error.rs`:

```rust
use thiserror::Error;
use wesichain_core::StoreError;

#[derive(Debug, Error)]
pub enum PineconeStoreError {
    #[error("invalid configuration: {0}")]
    Config(String),
    #[error("transport failure: {0}")]
    Transport(String),
    #[error("pinecone api error {status}: {message} (retry_after={retry_after_seconds:?}, namespace={namespace:?}, batch_size={batch_size:?})")]
    Api {
        status: u16,
        message: String,
        retry_after_seconds: Option<u64>,
        namespace: Option<String>,
        batch_size: Option<usize>,
    },
    #[error("malformed response: {0}")]
    Malformed(String),
    #[error("metadata reconstruction failed: missing or non-string text key '{text_key}'")]
    MissingTextKey { text_key: String },
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("batch mismatch: docs={docs}, embeddings={embeddings}")]
    BatchMismatch { docs: usize, embeddings: usize },
}

impl From<PineconeStoreError> for StoreError {
    fn from(value: PineconeStoreError) -> Self {
        StoreError::Internal(Box::new(value))
    }
}
```

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-pinecone error -v`

Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-pinecone/src/error.rs wesichain-pinecone/tests/error.rs wesichain-pinecone/src/lib.rs
git commit -m "feat(pinecone): add typed store errors"
```

---

### Task 3: Implement filter translation (typed + raw)

**Files:**
- Modify: `wesichain-pinecone/src/filter.rs`
- Test: `wesichain-pinecone/tests/filter.rs`

**Step 1: Write failing tests**

Create `wesichain-pinecone/tests/filter.rs`:

```rust
use serde_json::json;
use wesichain_core::MetadataFilter;
use wesichain_pinecone::filter::{PineconeFilter, to_pinecone_filter_json};

#[test]
fn converts_eq_filter() {
    let filter = PineconeFilter::Typed(MetadataFilter::Eq("source".to_string(), json!("tweet")));
    let out = to_pinecone_filter_json(&filter).unwrap();
    assert_eq!(out, json!({"source": {"$eq": "tweet"}}));
}

#[test]
fn converts_nested_all_any_filter() {
    let filter = PineconeFilter::Typed(MetadataFilter::All(vec![
        MetadataFilter::Eq("source".to_string(), json!("tweet")),
        MetadataFilter::Any(vec![
            MetadataFilter::In("lang".to_string(), vec![json!("en"), json!("id")]),
            MetadataFilter::Range {
                key: "score".to_string(),
                min: Some(json!(0.5)),
                max: None,
            },
        ]),
    ]));
    let out = to_pinecone_filter_json(&filter).unwrap();
    assert!(out.get("$and").is_some());
}

#[test]
fn raw_filter_passthrough() {
    let raw = json!({"$or": [{"source": {"$eq": "tweet"}}]});
    let out = to_pinecone_filter_json(&PineconeFilter::Raw(raw.clone())).unwrap();
    assert_eq!(out, raw);
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-pinecone filter -v`

Expected: FAIL (filter API not implemented).

**Step 3: Implement filter mapping**

Update `wesichain-pinecone/src/filter.rs`:

```rust
use serde_json::{json, Value};
use wesichain_core::MetadataFilter;

use crate::error::PineconeStoreError;

#[derive(Clone, Debug)]
pub enum PineconeFilter {
    Typed(MetadataFilter),
    Raw(Value),
}

pub fn to_pinecone_filter_json(filter: &PineconeFilter) -> Result<Value, PineconeStoreError> {
    match filter {
        PineconeFilter::Raw(value) => Ok(value.clone()),
        PineconeFilter::Typed(filter) => metadata_filter_to_json(filter),
    }
}

fn metadata_filter_to_json(filter: &MetadataFilter) -> Result<Value, PineconeStoreError> {
    Ok(match filter {
        MetadataFilter::Eq(key, value) => json!({ key: { "$eq": value } }),
        MetadataFilter::In(key, values) => json!({ key: { "$in": values } }),
        MetadataFilter::Range { key, min, max } => {
            let mut inner = serde_json::Map::new();
            if let Some(min) = min {
                inner.insert("$gte".to_string(), min.clone());
            }
            if let Some(max) = max {
                inner.insert("$lte".to_string(), max.clone());
            }
            Value::Object(serde_json::Map::from_iter([(key.clone(), Value::Object(inner))]))
        }
        MetadataFilter::All(filters) => {
            let list: Result<Vec<_>, _> = filters.iter().map(metadata_filter_to_json).collect();
            json!({ "$and": list? })
        }
        MetadataFilter::Any(filters) => {
            let list: Result<Vec<_>, _> = filters.iter().map(metadata_filter_to_json).collect();
            json!({ "$or": list? })
        }
    })
}
```

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-pinecone filter -v`

Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-pinecone/src/filter.rs wesichain-pinecone/tests/filter.rs
git commit -m "feat(pinecone): add typed and raw filter translation"
```

---

### Task 4: Implement document/vector mapping

**Files:**
- Modify: `wesichain-pinecone/src/types.rs`
- Modify: `wesichain-pinecone/src/mapper.rs`
- Test: `wesichain-pinecone/tests/mapper.rs`

**Step 1: Write failing tests**

Create `wesichain-pinecone/tests/mapper.rs`:

```rust
use std::collections::HashMap;

use serde_json::json;
use wesichain_core::Document;
use wesichain_pinecone::mapper::{doc_to_metadata, match_to_document};

#[test]
fn doc_to_metadata_preserves_fields_and_text_key() {
    let mut meta = HashMap::new();
    meta.insert("source".to_string(), json!("tweet"));
    let doc = Document {
        id: "d1".to_string(),
        content: "hello".to_string(),
        metadata: meta,
        embedding: None,
    };
    let out = doc_to_metadata(&doc, "text");
    assert_eq!(out.get("text"), Some(&json!("hello")));
    assert_eq!(out.get("source"), Some(&json!("tweet")));
}

#[test]
fn match_to_document_reads_text_key() {
    let metadata = json!({"text": "body", "source": "tweet"});
    let doc = match_to_document("id-1", &metadata, "text").unwrap();
    assert_eq!(doc.id, "id-1");
    assert_eq!(doc.content, "body");
    assert_eq!(doc.metadata.get("source"), Some(&json!("tweet")));
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-pinecone mapper -v`

Expected: FAIL (mapping helpers missing).

**Step 3: Implement mappers and Pinecone types**

In `wesichain-pinecone/src/types.rs`, define request/response structs (`UpsertRequest`, `PineconeVector`, `QueryRequest`, `QueryResponse`, `QueryMatch`, `DeleteRequest`, `IndexStatsResponse`).

In `wesichain-pinecone/src/mapper.rs` implement:

```rust
use std::collections::HashMap;

use serde_json::Value;
use wesichain_core::Document;

use crate::error::PineconeStoreError;

pub fn doc_to_metadata(doc: &Document, text_key: &str) -> HashMap<String, Value> {
    let mut metadata = doc.metadata.clone();
    metadata.insert(text_key.to_string(), Value::String(doc.content.clone()));
    metadata
}

pub fn match_to_document(
    id: &str,
    metadata: &Value,
    text_key: &str,
) -> Result<Document, PineconeStoreError> {
    let object = metadata.as_object().ok_or_else(|| {
        PineconeStoreError::Malformed("match metadata must be an object".to_string())
    })?;
    let text = object
        .get(text_key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| PineconeStoreError::MissingTextKey {
            text_key: text_key.to_string(),
        })?
        .to_string();

    let mut out = HashMap::new();
    for (k, v) in object {
        if k != text_key {
            out.insert(k.clone(), v.clone());
        }
    }

    Ok(Document {
        id: id.to_string(),
        content: text,
        metadata: out,
        embedding: None,
    })
}
```

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-pinecone mapper -v`

Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-pinecone/src/types.rs wesichain-pinecone/src/mapper.rs wesichain-pinecone/tests/mapper.rs
git commit -m "feat(pinecone): add metadata and response mappers"
```

---

### Task 5: Build HTTP client with contract tests

**Files:**
- Modify: `wesichain-pinecone/src/client.rs`
- Test: `wesichain-pinecone/tests/client.rs`

**Step 1: Write failing contract tests**

Create `wesichain-pinecone/tests/client.rs`:

```rust
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_pinecone::client::PineconeHttpClient;

#[tokio::test]
async fn upsert_sends_api_key_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/vectors/upsert"))
        .and(header("Api-Key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let client = PineconeHttpClient::new(server.uri(), "test-key".to_string()).unwrap();
    let payload = json!({"vectors": [], "namespace": "prod"});
    client.post_json("/vectors/upsert", &payload).await.unwrap();
}

#[tokio::test]
async fn maps_api_error_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/query"))
        .respond_with(
            ResponseTemplate::new(429).set_body_json(json!({"message": "rate limit"})),
        )
        .mount(&server)
        .await;

    let client = PineconeHttpClient::new(server.uri(), "test-key".to_string()).unwrap();
    let err = client.post_json("/query", &json!({})).await.unwrap_err();
    assert!(err.to_string().contains("429"));
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-pinecone client -v`

Expected: FAIL (HTTP client API missing).

**Step 3: Implement HTTP client**

Implement in `wesichain-pinecone/src/client.rs`:

- `PineconeHttpClient::new(base_url, api_key)` validates URL and builds reqwest client.
- `post_json(path, payload)` with `Api-Key` and JSON headers.
- `post_typed<Req, Resp>(...)` generic helper.
- status/error-body parsing to `PineconeStoreError::Api`.
- optional `Retry-After` extraction into error.

Example core method:

```rust
pub async fn post_typed<Req, Resp>(&self, path: &str, payload: &Req) -> Result<Resp, PineconeStoreError>
where
    Req: serde::Serialize,
    Resp: serde::de::DeserializeOwned,
{
    let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
    let response = self
        .http
        .post(url)
        .header("Api-Key", &self.api_key)
        .header("Content-Type", "application/json")
        .json(payload)
        .send()
        .await
        .map_err(|err| PineconeStoreError::Transport(err.to_string()))?;
    // map status + parse body
    // parse success json into Resp
}
```

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-pinecone client -v`

Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-pinecone/src/client.rs wesichain-pinecone/tests/client.rs
git commit -m "feat(pinecone): add data-plane http client"
```

---

### Task 6: Implement builder and configuration defaults

**Files:**
- Modify: `wesichain-pinecone/src/config.rs`
- Modify: `wesichain-pinecone/src/store.rs`
- Modify: `wesichain-pinecone/src/lib.rs`
- Test: `wesichain-pinecone/tests/builder.rs`

**Step 1: Write failing tests**

Create `wesichain-pinecone/tests/builder.rs`:

```rust
use wesichain_core::{Embedding, EmbeddingError};
use wesichain_pinecone::PineconeVectorStore;

#[derive(Clone)]
struct DummyEmbedding;

#[async_trait::async_trait]
impl Embedding for DummyEmbedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> { Ok(vec![0.1, 0.2]) }
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|_| vec![0.1, 0.2]).collect())
    }
    fn dimension(&self) -> usize { 2 }
}

#[tokio::test]
async fn builder_sets_default_text_key() {
    let store = PineconeVectorStore::builder(DummyEmbedding)
        .base_url("https://example.test")
        .api_key("key")
        .build()
        .await
        .unwrap();
    assert_eq!(store.text_key(), "text");
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-pinecone builder -v`

Expected: FAIL (builder/store methods missing).

**Step 3: Implement builder and store config**

Implement in `wesichain-pinecone/src/config.rs`:
- `PineconeStoreBuilder<E>` with fields: `embedder`, `base_url`, `api_key`, `namespace`, `text_key`, `index_name`, `validate_dimension`.
- fluent setters: `.base_url(...)`, `.api_key(...)`, `.namespace(...)`, `.text_key(...)`, `.index_name(...)`, `.validate_dimension(...)`.
- optional env helpers: `.api_key_from_env("PINECONE_API_KEY")`, `.base_url_from_env("PINECONE_BASE_URL")`.
- `build().await -> PineconeVectorStore<E>`.

Add minimal constructor/getter support in `store.rs` for testability (`text_key()` accessor).

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-pinecone builder -v`

Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-pinecone/src/config.rs wesichain-pinecone/src/store.rs wesichain-pinecone/src/lib.rs wesichain-pinecone/tests/builder.rs
git commit -m "feat(pinecone): add fluent builder and defaults"
```

---

### Task 7: Implement add/upsert path (`add_documents`, `from_documents`)

**Files:**
- Modify: `wesichain-pinecone/src/store.rs`
- Modify: `wesichain-pinecone/src/types.rs`
- Test: `wesichain-pinecone/tests/add_documents.rs`

**Step 1: Write failing tests**

Create `wesichain-pinecone/tests/add_documents.rs`:

```rust
use std::collections::HashMap;

use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_core::{Document, Embedding, EmbeddingError};
use wesichain_pinecone::PineconeVectorStore;

#[derive(Clone)]
struct FixedEmbedding;

#[async_trait::async_trait]
impl Embedding for FixedEmbedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> { Ok(vec![0.1, 0.2, 0.3]) }
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect())
    }
    fn dimension(&self) -> usize { 3 }
}

#[tokio::test]
async fn add_documents_embeds_and_upserts() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/vectors/upsert"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let store = PineconeVectorStore::builder(FixedEmbedding)
        .base_url(server.uri())
        .api_key("key")
        .build()
        .await
        .unwrap();

    let doc = Document {
        id: "doc-1".to_string(),
        content: "hello".to_string(),
        metadata: HashMap::new(),
        embedding: None,
    };

    store.add_documents(vec![doc], None).await.unwrap();
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-pinecone add_documents -v`

Expected: FAIL (`add_documents` not implemented).

**Step 3: Implement write path**

In `store.rs` implement:
- `add_documents(&self, docs: Vec<Document>, ids: Option<Vec<String>>) -> Result<(), StoreError>`.
- embedding with `embed_batch`.
- batch/dimension validation.
- vector payload mapping with `text_key`.
- ID behavior: prefer explicit IDs, fallback to document IDs if non-empty, else generate `Uuid::new_v4()`.
- `from_documents(...)` convenience wrapper.

Add tracing span:

```rust
let span = tracing::info_span!(
    "pinecone_upsert",
    namespace = ?self.namespace,
    batch_size = docs.len(),
    text_key = %self.text_key,
);
let _guard = span.enter();
```

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-pinecone add_documents -v`

Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-pinecone/src/store.rs wesichain-pinecone/src/types.rs wesichain-pinecone/tests/add_documents.rs
git commit -m "feat(pinecone): implement add_documents upsert flow"
```

---

### Task 8: Implement query/search APIs and score-return variant

**Files:**
- Modify: `wesichain-pinecone/src/store.rs`
- Modify: `wesichain-pinecone/src/filter.rs`
- Test: `wesichain-pinecone/tests/search.rs`

**Step 1: Write failing tests**

Create `wesichain-pinecone/tests/search.rs`:

```rust
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_core::{Embedding, EmbeddingError, MetadataFilter};
use wesichain_pinecone::PineconeVectorStore;

#[derive(Clone)]
struct FixedEmbedding;

#[async_trait::async_trait]
impl Embedding for FixedEmbedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> { Ok(vec![0.9, 0.1]) }
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|_| vec![0.9, 0.1]).collect())
    }
    fn dimension(&self) -> usize { 2 }
}

#[tokio::test]
async fn similarity_search_returns_documents() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "matches": [
                {
                    "id": "doc-1",
                    "score": 0.88,
                    "metadata": {"text": "hello", "source": "tweet"}
                }
            ]
        })))
        .mount(&server)
        .await;

    let store = PineconeVectorStore::builder(FixedEmbedding)
        .base_url(server.uri())
        .api_key("key")
        .build()
        .await
        .unwrap();

    let docs = store
        .similarity_search("query", 3, Some(MetadataFilter::Eq("source".to_string(), json!("tweet"))))
        .await
        .unwrap();

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].content, "hello");
}

#[tokio::test]
async fn similarity_search_with_score_returns_score() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "matches": [{"id": "doc-1", "score": 0.66, "metadata": {"text": "hello"}}]
        })))
        .mount(&server)
        .await;

    let store = PineconeVectorStore::builder(FixedEmbedding)
        .base_url(server.uri())
        .api_key("key")
        .build()
        .await
        .unwrap();

    let out = store.similarity_search_with_score("query", 2, None).await.unwrap();
    assert_eq!(out[0].1, 0.66);
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-pinecone search -v`

Expected: FAIL (`similarity_search` APIs missing).

**Step 3: Implement search APIs**

In `store.rs` implement:
- `similarity_search(&self, query: &str, k: usize, filter: Option<MetadataFilter>) -> Result<Vec<Document>, StoreError>`
- `similarity_search_with_score(...) -> Result<Vec<(Document, f32)>, StoreError>`
- internal `query_by_vector(...)` for shared logic.

Behavior:
- embed query via `embed`.
- include `include_metadata=true`.
- apply typed filter translation or raw filter when provided via overloaded method/enum.
- reconstruct `Document` with `text_key`.

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-pinecone search -v`

Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-pinecone/src/store.rs wesichain-pinecone/src/filter.rs wesichain-pinecone/tests/search.rs
git commit -m "feat(pinecone): add similarity_search APIs"
```

---

### Task 9: Implement delete API and `VectorStore` trait integration

**Files:**
- Modify: `wesichain-pinecone/src/store.rs`
- Modify: `wesichain-pinecone/src/lib.rs`
- Test: `wesichain-pinecone/tests/vector_store.rs`

**Step 1: Write failing tests**

Create `wesichain-pinecone/tests/vector_store.rs`:

```rust
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_core::{Document, Embedding, EmbeddingError, VectorStore};
use wesichain_pinecone::PineconeVectorStore;

#[derive(Clone)]
struct FixedEmbedding;

#[async_trait::async_trait]
impl Embedding for FixedEmbedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> { Ok(vec![0.2, 0.3]) }
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|_| vec![0.2, 0.3]).collect())
    }
    fn dimension(&self) -> usize { 2 }
}

#[tokio::test]
async fn vector_store_trait_search_and_delete_work() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "matches": [{"id":"d1","score":0.9,"metadata":{"text":"hello"}}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/vectors/delete"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let store = PineconeVectorStore::builder(FixedEmbedding)
        .base_url(server.uri())
        .api_key("key")
        .build()
        .await
        .unwrap();

    let out = store.search(&[0.2, 0.3], 3, None).await.unwrap();
    assert_eq!(out.len(), 1);

    store.delete(&["d1".to_string()]).await.unwrap();
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-pinecone vector_store -v`

Expected: FAIL (`VectorStore` impl incomplete).

**Step 3: Implement trait and delete wrappers**

In `store.rs`:
- implement `VectorStore` for `PineconeVectorStore<E>`.
- `add` delegates to `add_documents(docs, None)`.
- `search` uses provided embedding directly and returns `Vec<SearchResult>`.
- `delete` maps to `/vectors/delete`.

Also expose LangChain-style wrapper:

```rust
pub async fn delete_ids(&self, ids: Vec<String>) -> Result<(), StoreError> { ... }
pub async fn delete(&self, ids: Vec<String>) -> Result<(), StoreError> { ... }
```

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-pinecone vector_store -v`

Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-pinecone/src/store.rs wesichain-pinecone/src/lib.rs wesichain-pinecone/tests/vector_store.rs
git commit -m "feat(pinecone): implement vectorstore trait and delete"
```

---

### Task 10: Add optional dimension validation and tracing coverage

**Files:**
- Modify: `wesichain-pinecone/src/store.rs`
- Modify: `wesichain-pinecone/src/types.rs`
- Test: `wesichain-pinecone/tests/dimension_validation.rs`

**Step 1: Write failing tests**

Create `wesichain-pinecone/tests/dimension_validation.rs`:

```rust
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_core::{Embedding, EmbeddingError};
use wesichain_pinecone::PineconeVectorStore;

#[derive(Clone)]
struct Dim2Embedding;

#[async_trait::async_trait]
impl Embedding for Dim2Embedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> { Ok(vec![0.1, 0.2]) }
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|_| vec![0.1, 0.2]).collect())
    }
    fn dimension(&self) -> usize { 2 }
}

#[tokio::test]
async fn dimension_check_does_not_fail_build_when_mismatch() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/describe_index_stats"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"dimension": 3})))
        .mount(&server)
        .await;

    let result = PineconeVectorStore::builder(Dim2Embedding)
        .base_url(server.uri())
        .api_key("key")
        .validate_dimension(true)
        .build()
        .await;

    assert!(result.is_ok());
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-pinecone dimension_validation -v`

Expected: FAIL (describe/stats validation path missing).

**Step 3: Implement optional validation**

Implement in `store.rs`:
- call `/describe_index_stats` during `build` when `validate_dimension=true`.
- if returned dimension mismatches embedder dimension: emit `tracing::warn!` and continue.
- if describe endpoint fails: emit `tracing::warn!` and continue (do not hard-fail).

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-pinecone dimension_validation -v`

Expected: PASS.

**Step 5: Commit**

```bash
git add wesichain-pinecone/src/store.rs wesichain-pinecone/src/types.rs wesichain-pinecone/tests/dimension_validation.rs
git commit -m "feat(pinecone): add optional dimension validation"
```

---

### Task 11: Docs, example, and migration table (OpenAI + Google)

**Files:**
- Create: `wesichain-pinecone/README.md`
- Create: `wesichain-pinecone/examples/pinecone_rag.rs`
- Modify: `wesichain-pinecone/src/lib.rs`

**Step 1: Write the docs first (failing doctest if needed)**

Add crate-level docs in `wesichain-pinecone/src/lib.rs` and README quickstart with required env vars:
- `PINECONE_API_KEY`
- `PINECONE_BASE_URL`
- `PINECONE_NAMESPACE` (optional)

Add migration table rows:
- Python `PineconeVectorStore(...).add_documents(...)` -> Rust equivalent.
- Python `similarity_search(...)` -> Rust equivalent.
- Python `similarity_search_with_score(...)` -> Rust equivalent.
- Python Google embeddings row -> Rust Google embeddings row.

**Step 2: Add runnable example**

Create `wesichain-pinecone/examples/pinecone_rag.rs` with:
- store builder usage,
- small document ingest,
- query and print results,
- `tracing_subscriber::fmt::init()`.

**Step 3: Verify docs and examples compile**

Run: `cargo test -p wesichain-pinecone --doc`

Expected: PASS.

Run: `cargo build -p wesichain-pinecone --examples`

Expected: PASS.

**Step 4: Commit**

```bash
git add wesichain-pinecone/README.md wesichain-pinecone/examples/pinecone_rag.rs wesichain-pinecone/src/lib.rs
git commit -m "docs(pinecone): add quickstart and migration guide"
```

---

### Task 12: Full verification and polish

**Files:**
- Modify: `wesichain-pinecone/Cargo.toml`
- Modify: `wesichain-pinecone/src/*.rs` (only if lint/type/test issues)

**Step 1: Run package tests**

Run: `cargo test -p wesichain-pinecone -v`

Expected: PASS.

**Step 2: Run full workspace tests**

Run: `cargo test`

Expected: PASS.

**Step 3: Commit final cleanup**

```bash
git add wesichain-pinecone/Cargo.toml wesichain-pinecone/src wesichain-pinecone/tests
git commit -m "test(pinecone): finalize verification and cleanup"
```

---

## Validation Checklist

- [ ] `wesichain-pinecone` is a standalone workspace crate.
- [ ] `PineconeVectorStore<E: Embedding>` works with external embedders.
- [ ] `text_key` defaults to `text` and reconstruction preserves metadata.
- [ ] Typed `MetadataFilter` translation works for `Eq`, `In`, `Range`, `All`, `Any`.
- [ ] Raw filter passthrough is supported.
- [ ] `add_documents`, `similarity_search`, `similarity_search_with_score`, `delete` are implemented.
- [ ] `VectorStore` trait is implemented.
- [ ] HTTP errors include status + provider message + retry hints.
- [ ] Dimension validation is optional and warning-only.
- [ ] README includes env vars and migration table with Google example.
- [ ] Example compiles.
- [ ] Full workspace tests pass.

## Suggested verification commands

- `cargo test -p wesichain-pinecone -v`
- `cargo test -p wesichain-pinecone --doc`
- `cargo build -p wesichain-pinecone --examples`
- `cargo test`
