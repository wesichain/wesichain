# Wesichain Phase 6 Retrieval + Embeddings Design

Date: 2026-02-03
Status: Draft

## Goals
- Provide a lean, end-to-end RAG path: documents -> embeddings -> in-memory retrieval -> graph integration.
- Keep `wesichain-core` dependency-light and stable for v0.1.
- Make embeddings and vector stores pluggable and feature-gated.
- Deliver runnable examples and benchmarks that prove Rust performance and memory wins.

## Non-goals
- Hybrid search (BM25/RRF), reranking, or HNSW indexing in Phase 6.
- Qdrant or other external vector DB integrations.
- Streaming retrieval or multi-modal embeddings.
- Built-in retries for provider or store errors.

## Scope (Phase 6 Lean MVP)
- Core traits and types: `Document`, `Embedding`, `VectorStore`, `MetadataFilter`.
- `wesichain-embeddings` providers: OpenAI and Ollama (Candle optional).
- `wesichain-retrieval`: in-memory store, loaders, splitter, indexer, retriever.
- `wesichain-graph`: `RetrieverNode` + state traits.
- Examples and benchmarks demonstrating indexing and query latency.

## Architecture
Dependency direction mirrors Phase 5 and keeps core stable:

```
wesichain-core
    <- wesichain-embeddings
        <- wesichain-retrieval
            <- wesichain-graph
```

Core defines interfaces only; providers and stores are in downstream crates. This enables “bring your own embeddings” without dragging provider deps into core.

## Core Interfaces (wesichain-core)

### Document
```
pub struct Document {
    pub id: String,
    pub content: String,
    pub metadata: HashMap<String, Value>,
    pub embedding: Option<Vec<f32>>,
}
```

- `id` is required to keep core free of UUID dependencies.
- `embedding` is optional to support precomputed vectors.

### Embedding
```
#[async_trait::async_trait]
pub trait Embedding: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
    fn dimension(&self) -> usize;
}
```

- Batch is the primary path for indexing performance.
- `async-trait` is retained for object safety (`Arc<dyn Embedding>`).

### VectorStore
```
#[async_trait::async_trait]
pub trait VectorStore: Send + Sync {
    async fn add(&self, docs: Vec<Document>) -> Result<(), StoreError>;
    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, StoreError>;
    async fn delete(&self, ids: &[String]) -> Result<(), StoreError>;
}

pub struct SearchResult {
    pub document: Document,
    pub score: f32,
}
```

- `&self` keeps the trait object-safe; stores use internal locking.
- Score convention: cosine similarity in [-1.0, 1.0], higher is better.
- Filters are post-similarity; results may be fewer than `top_k`.

### MetadataFilter
```
pub enum MetadataFilter {
    Eq(String, Value),
    In(String, Vec<Value>),
    Range { key: String, min: Option<Value>, max: Option<Value> },
    All(Vec<MetadataFilter>),
    Any(Vec<MetadataFilter>),
}
```

- Only primitive values are supported in Phase 6 (string/number/bool/null).

### Errors
```
pub enum EmbeddingError {
    InvalidResponse(String),
    RateLimited,
    Timeout,
    Provider(String),
    Other(Box<dyn std::error::Error + Send + Sync>),
}

pub enum StoreError {
    DimensionMismatch { expected: usize, got: usize },
    InvalidId(String),
    Internal(String),
}
```

- Errors are explicit and fail-fast; no retries in core paths.

## Components

### wesichain-embeddings
- `OpenAiEmbedding` (feature `openai`) using `async-openai`.
- `OllamaEmbedding` (feature `ollama`) using `ollama-rs`.
- `CandleEmbedding` (feature `candle`, optional and heavy).
- Providers expose batch embeddings and fixed dimension at construction.

### wesichain-retrieval
- `InMemoryVectorStore`:
  - `tokio::sync::RwLock` for read-heavy workloads.
  - Stores `Vec<Document>` plus parallel `Vec<Vec<f32>>` embeddings.
  - Caches dimension on first add; validates on add/search.
  - Filters applied post-similarity.
  - Embeddings are stripped from stored `Document`; search returns `embedding: None`.
  - Delete is idempotent and implemented via tombstoning; memory reclaimed on drop.
- `Indexer`:
  - Requires valid IDs; rejects empty IDs unless `uuid` feature fills them.
  - Uses `embed_batch`, attaches embeddings, then calls `store.add`.
- `TextLoader` (default) and `PdfLoader` (feature `pdf`).
- `TextSplitter` (simple chunk size + overlap) lives in retrieval.
- `Retriever` helper embeds query then calls `store.search`.

### wesichain-graph
- `RetrieverNode` integrates retrieval into graph execution.

## Data Flow

### Indexing
1. Load documents (Text/PDF loader).
2. Split into chunks via `TextSplitter` (optional).
3. `Indexer` embeds chunks in batch and attaches embeddings.
4. `VectorStore::add` persists documents + vectors.

### Retrieval
1. `Retriever` (or `RetrieverNode`) embeds the query.
2. `VectorStore::search` returns `SearchResult` scored by cosine similarity.
3. Optional `score_threshold` applied locally (`score >= threshold`).
4. Results written into state as `Vec<Document>`.

## Graph Integration
State traits in `wesichain-core`:

```
pub trait HasQuery {
    fn query(&self) -> &str;
}

pub trait HasRetrievedDocs {
    fn set_retrieved_docs(&mut self, docs: Vec<Document>);
}

pub trait HasMetadataFilter {
    fn metadata_filter(&self) -> Option<MetadataFilter>;
}
```

`RetrieverNode` owns `Arc<dyn Embedding>` and `Arc<dyn VectorStore>`, plus `top_k` and optional `score_threshold`. It embeds the query, calls `search`, applies threshold, and replaces the retrieved docs in state. Errors propagate and halt graph execution.

## Error Handling
- Embedding and store errors fail fast and propagate upward.
- Empty results (no hits or restrictive filters) are valid and return an empty list.
- Graph mapping: errors become `GraphError::NodeFailed` (or `GraphError::RetrievalFailed` if added later).
- Lock poisoning and internal store failures map to `StoreError::Internal`.

## Testing
- Core: serde round-trip for `Document` and `MetadataFilter`, error formatting.
- Retrieval:
  - Cosine ranking order with fixed vectors.
  - Filter semantics (Eq/In/Range/All/Any).
  - Dimension mismatch on add/search.
  - Invalid ID handling and delete idempotency.
  - Embedding stripping (`Document.embedding == None` on search result).
  - Score threshold inclusive boundary (`score >= threshold`).
- Embeddings:
  - Feature-gated provider tests with wiremock.
  - One `*_live` test per provider (manual only).
- Graph:
  - `RetrieverNode` state update, threshold filtering, error propagation.

## Examples & Benchmarks
- `wesichain-retrieval/examples/index_and_query.rs` uses a deterministic `HashEmbedder` so it runs without API keys.
- Feature-gated variants for OpenAI and Ollama (`--features openai` / `ollama`).
- `wesichain-graph/examples/retriever_node.rs` shows graph integration and prints retrieval timing.

Benchmarks:
- Criterion benchmarks for batch indexing and query latency at 1k and 10k docs.
- Memory measurement tool reports RSS before/after indexing.
- Python baseline script uses matching chunking and embeddings for fair comparison.
- Hybrid/HNSW explicitly excluded in Phase 6 benchmark claims.

## Future Work
- Hybrid search (BM25 + RRF) and reranking.
- HNSW or other ANN indexes for large corpora.
- Qdrant integration and advanced filters.
- Rehydrating embeddings in results (optional).
- Additional loaders and chunkers.
