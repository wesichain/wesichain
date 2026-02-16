# wesichain-qdrant

Qdrant vector store integration for Wesichain.

## Features

- `QdrantVectorStore` implementation of `wesichain_core::VectorStore`
- Builder API with optional API key and cloud URL warning
- Add/search/delete operations with typed error mapping
- Metadata filter translation (`Eq`, `In`, `Range`, `All`, `Any`)
- Contract tests and migration parity test harness

## Quick Start

```rust
use wesichain_core::VectorStore;
use wesichain_qdrant::QdrantVectorStore;

let store = QdrantVectorStore::builder()
    .base_url("http://127.0.0.1:6333")
    .collection("docs")
    .build()?;

let results = store.search(&[0.1, 0.2, 0.3], 5, None).await?;
println!("hits={}", results.len());
```

See `examples/rag_integration.rs` for an end-to-end migration-oriented flow.
