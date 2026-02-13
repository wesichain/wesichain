# Wesichain Pinecone Vector Store Design

Date: 2026-02-06
Status: Validated

## Goal
Add a production-ready `wesichain-pinecone` crate that mirrors common Python `PineconeVectorStore` ergonomics while staying Rust-idiomatic, async-first, and dependency-lean.

## Scope

### In scope (v0)
- New workspace crate: `wesichain-pinecone`.
- Data-plane operations only: upsert, query, delete, optional stats check for dimension validation.
- External embedding injection (`E: Embedding`) with Google/OpenAI/Ollama/local compatibility.
- Hybrid API surface:
  - Rust-idiomatic core methods.
  - Compatibility wrapper methods.
- Typed metadata filters with raw JSON fallback.
- Tracing spans and contextual error messages.
- Unit tests, HTTP contract tests, and ignored live tests.
- Migration docs and examples.

### Out of scope (v0)
- Control-plane lifecycle APIs (create/list/describe/delete index resources).
- Built-in retry/backoff policy.
- Sparse/hybrid retrieval and reranking.
- Pinecone integrated inference.

## Approach Options and Decision

### Option A: New crate `wesichain-pinecone` (chosen)
- Pros: mirrors established Python packaging patterns, clean dependency boundaries, independent release cadence, strong migration discoverability.
- Cons: small initial setup overhead.

### Option B: Add Pinecone backend to `wesichain-retrieval` behind a feature
- Pros: slightly faster first commit.
- Cons: bloats retrieval crate, weaker modular story.

### Option C: internal generic Pinecone client crate + wrappers
- Pros: long-term flexibility.
- Cons: over-engineered for MVP (YAGNI).

Decision: Option A.

## Architecture

`wesichain-pinecone` is a standalone integration crate with three internal layers:

1. `PineconeVectorStore<E>` (public orchestration layer)
   - Owns configuration and public API.
   - Generic over `E: Embedding`.
   - Implements `VectorStore` compatibility behavior.

2. `PineconeHttpClient` (internal transport)
   - Thin typed `reqwest` client for Pinecone data-plane endpoints.
   - No business logic beyond request/response wiring.

3. Mapping/serialization module (internal)
   - `Document` <-> Pinecone metadata/vector conversion.
   - Wesichain typed filter -> Pinecone filter JSON conversion.

This separation keeps logic testable, avoids serde-heavy store methods, and supports future growth without API churn.

## Public API Surface

Builder-first initialization:

```rust
let store = PineconeVectorStore::builder(embedder)
    .base_url("https://<index-host>")
    .api_key("<key>")
    .index_name("optional-name-for-logs")
    .namespace("prod")
    .text_key("text")
    .validate_dimension(false)
    .build()
    .await?;
```

Notes:
- `base_url` is a full HTTPS endpoint to avoid host-format ambiguity.
- `namespace` defaults to omitted (Pinecone default behavior).
- `text_key` defaults to `"text"`.
- `validate_dimension` defaults to `false` and is warning-only when enabled.

Hybrid methods:

- Rust-idiomatic core:
  - `add(docs, ids)`
  - `search(query, k, filter)`
  - `search_with_score(query, k, filter)`
  - `delete_ids(ids)`

- Compatibility wrappers:
  - `add_documents(docs, ids)`
  - `similarity_search(query, k, filter)`
  - `similarity_search_with_score(query, k, filter)`
  - `delete(ids)`

Convenience constructor:
- `from_documents(...)` creates store and ingests docs in one flow.

## Data Flow

### Write path (`add_documents`)
1. Extract `page_content` text from each `Document`.
2. Call `embedder.embed_batch(...)`.
3. Validate embedding count equals doc count.
4. Validate per-vector dimensions are consistent with embedder output.
5. Build Pinecone vector payload:
   - `id`: supplied ID or generated UUID.
   - `values`: embedding vector.
   - `metadata`: includes content at `metadata[text_key]` and preserves all other metadata fields.
6. Upsert to Pinecone `/vectors/upsert`.

### Read path (`similarity_search`)
1. Embed query via `embedder.embed(query)`.
2. Build query payload: `vector`, `top_k`, `include_metadata=true`, optional `namespace`, optional filter.
3. Call `/query`.
4. Reconstruct `Document` from match metadata:
   - `page_content <- metadata[text_key]`
   - `metadata <- metadata minus text_key`
5. Return ordered documents.

### Scored read path (`similarity_search_with_score`)
- Same as read path, but return `(Document, f32)` with Pinecone similarity score.

### Delete path
- `delete(ids)` maps to `/vectors/delete` with optional namespace.

## Filter Strategy

Dual-mode filtering for type safety and flexibility:

1. Typed filters (default)
   - Built from Wesichain `MetadataFilter` (`Eq`, `In`, `Range`, `All`, `Any`).
   - Serialized to Pinecone operators (`$eq`, `$in`, `$gte/$lte`, `$and`, `$or`).

2. Raw JSON fallback
   - Accept `serde_json::Value` for advanced Pinecone-native expressions not covered in typed model.

This keeps common flows safe and discoverable while avoiding hard blockers for advanced users.

## Error Handling and Validation

Public methods return `StoreError` (trait compatibility). Internally, the crate uses `PineconeStoreError` with conversion to `StoreError::Internal(...)` and rich context.

Error classes:
- Invalid configuration (missing/invalid `base_url`, `api_key`).
- Transport errors (`reqwest`).
- API errors with status/body context.
- Malformed response payloads.
- Batch mismatch (documents vs embeddings).
- Dimension mismatch.
- Metadata reconstruction errors (`text_key` missing/non-string).

Status-aware reporting:
- `400`: invalid request/filter payload.
- `401/403`: auth/permission issues.
- `404`: wrong index endpoint/path.
- `429`: rate limited (include `Retry-After` hint when present).
- `5xx`: provider-side failures.

Validation policy:
- Strict on correctness invariants (batch and dimension consistency).
- Tolerant on index-dimension precheck (`validate_dimension=true` warns, does not hard-fail by default).

## Observability

Add `tracing` spans for key operations:
- `pinecone_upsert`
- `pinecone_query`
- `pinecone_delete`
- `pinecone_describe_index_stats` (optional)

Span fields:
- index name (if provided for logs), base URL host, namespace, batch size/top_k, latency.

This provides production visibility without forcing external observability dependencies.

## Testing Plan

### 1) Unit tests (no network)
- Metadata/text mapping round trips.
- Missing/invalid `text_key` behavior.
- Typed filter serialization.
- Batch and dimension validation paths.

### 2) HTTP contract tests (mock server)
- Exact request shape for upsert/query/delete.
- Header assertions (`Api-Key`, JSON content type).
- Namespace omitted vs included behavior.
- Status mapping for `400/401/403/404/429/5xx`.
- Malformed body parsing and error fallback.
- Score and ordering preservation in `similarity_search_with_score`.

### 3) Ignored live integration tests (env-gated)
- Required env vars:
  - `PINECONE_API_KEY`
  - `PINECONE_BASE_URL`
  - `PINECONE_INDEX` (optional if only URL is used operationally)
- End-to-end upsert/query/delete with deterministic test embedder.

## Migration UX and Documentation Deliverables

Required deliverables:
- `wesichain-pinecone` README with quickstart.
- `examples/pinecone_rag.rs` showing ingest + retrieval.
- Python baseline -> Rust Wesichain mapping table.
- Common gotchas section:
  - full endpoint URL required,
  - namespace omission behavior,
  - `text_key` reconstruction semantics,
  - typical 401/404/429 failures.
- Tracing quickstart with `RUST_LOG` examples.

Migration mapping must explicitly cover:
- `add_documents`
- `similarity_search`
- `similarity_search_with_score`
- `delete`

Include one example using Google embeddings in addition to OpenAI to support migration targets already identified in milestone planning.

## Dependencies

Core runtime deps (target minimal):
- `wesichain-core`
- `reqwest`
- `serde`, `serde_json`
- `uuid`
- `tracing`
- `thiserror`

Dev deps:
- `wiremock` (or `httpmock`)
- `tokio` test runtime

No Pinecone SDK dependency in v0.

## Definition of Done (v0)

- Crate added to workspace and compiles cleanly.
- Public API finalized with hybrid naming.
- Unit and mock contract tests pass in CI.
- Ignored live tests pass when credentials are provided.
- Example compiles and demonstrates ingest + retrieval.
- Migration docs are present and accurate.
- Non-goals remain deferred and documented.

## Post-v0 Roadmap

1. Optional retry/backoff strategy hooks.
2. Control-plane helper APIs.
3. Sparse/hybrid retrieval support.
4. Optional Pinecone integrated inference path (reevaluate when Rust ecosystem maturity improves).
