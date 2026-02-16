# LangChain to Wesichain Qdrant Migration Guide

This guide shows the same Qdrant core flow in LangChain (Python) and Wesichain (Rust): build a vector store, add embedded documents, query/search, and delete by id.

## Side-by-Side Core Flow

### LangChain (Python)

```python
from langchain_qdrant import QdrantVectorStore
from langchain_openai import OpenAIEmbeddings

embeddings = OpenAIEmbeddings(model="text-embedding-3-small")
store = QdrantVectorStore.from_existing_collection(
    embedding=embeddings,
    url="http://127.0.0.1:6333",
    collection_name="wesichain_docs",
)

texts = [
    "Wesichain is a Rust-native LLM framework focused on graph and agent workflows.",
    "Qdrant stores vectors and metadata for similarity retrieval.",
]
ids = ["doc-1", "doc-2"]

store.add_texts(texts=texts, ids=ids)
hits = store.similarity_search_with_score("What is Wesichain focused on?", k=2)
store.delete(ids=ids)
```

### Wesichain (Rust)

```rust
use std::collections::HashMap;

use serde_json::json;
use wesichain_core::{Document, VectorStore};
use wesichain_qdrant::QdrantVectorStore;

let store = QdrantVectorStore::builder()
    .base_url("http://127.0.0.1:6333")
    .collection("wesichain_docs")
    .build()?;

let docs = vec![
    Document {
        id: "doc-1".to_string(),
        content: "Wesichain is a Rust-native LLM framework focused on graph and agent workflows."
            .to_string(),
        metadata: HashMap::from([("source".to_string(), json!("guide"))]),
        embedding: Some(vec![0.99, 0.01, 0.0]),
    },
    Document {
        id: "doc-2".to_string(),
        content: "Qdrant stores vectors and metadata for similarity retrieval.".to_string(),
        metadata: HashMap::from([("source".to_string(), json!("guide"))]),
        embedding: Some(vec![0.70, 0.30, 0.0]),
    },
];

store.add(docs).await?;

let hits = store.search(&[0.98, 0.02, 0.0], 2, None).await?;
store.delete(&["doc-1".to_string(), "doc-2".to_string()]).await?;
```

The runnable Rust implementation is in `wesichain-qdrant/examples/rag_integration.rs`.

## Verification Commands

Run these from the workspace root:

```bash
cargo test -p wesichain-qdrant migration_parity -- --nocapture
cargo check -p wesichain-qdrant --examples
cargo test -p wesichain-qdrant --tests -- --nocapture
```

Optional local run against Qdrant:

```bash
QDRANT_URL=http://127.0.0.1:6333 \
QDRANT_COLLECTION=wesichain_docs \
cargo run -p wesichain-qdrant --example rag_integration
```
