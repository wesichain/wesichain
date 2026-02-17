# wesichain-weaviate

Weaviate vector store integration for Wesichain.

## Features

- `WeaviateVectorStore` implementation of `wesichain_core::VectorStore`
- Builder API with required `base_url` and `class_name`, optional `api_key`
- Add/search/delete operations with class bootstrap on add
- Metadata preservation and descending-score search results
- Contract tests with live-gated roundtrip and deterministic mock coverage

## Quick Start

```rust
use wesichain_core::VectorStore;
use wesichain_weaviate::WeaviateVectorStore;

let store = WeaviateVectorStore::builder()
    .base_url("http://127.0.0.1:8080")
    .class_name("Docs")
    .build()?;

let results = store.search(&[0.1, 0.2, 0.3], 5, None).await?;
println!("hits={}", results.len());
```
