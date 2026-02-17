# LangChain to Wesichain Weaviate Migration Guide

This guide shows the same Weaviate core flow in LangChain (Python) and Wesichain (Rust): build a vector store, add embedded documents, query/search, and delete by id.

## Side-by-Side Core Flow

### LangChain (Python)

Note: this snippet uses the Weaviate Python client v4 style (`weaviate.connect_to_local(...)`) to avoid v3/v4 API ambiguity.

```python
import weaviate
from langchain_openai import OpenAIEmbeddings
from langchain_weaviate import WeaviateVectorStore

client = weaviate.connect_to_local(host="127.0.0.1", port=8080, grpc_port=50051)
embeddings = OpenAIEmbeddings(model="text-embedding-3-small")

store = WeaviateVectorStore(
    client=client,
    index_name="WesichainDocs",
    text_key="text",
    embedding=embeddings,
)

texts = [
    "Wesichain is a Rust-native LLM framework focused on graph and agent workflows.",
    "Weaviate stores vectors and metadata for similarity retrieval.",
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
use wesichain_weaviate::WeaviateVectorStore;

let store = WeaviateVectorStore::builder()
    .base_url("http://127.0.0.1:8080")
    .class_name("WesichainDocs")
    .auto_create_class(true)
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
        content: "Weaviate stores vectors and metadata for similarity retrieval.".to_string(),
        metadata: HashMap::from([("source".to_string(), json!("guide"))]),
        embedding: Some(vec![0.70, 0.30, 0.0]),
    },
];

store.add(docs).await?;

let hits = store.search(&[0.98, 0.02, 0.0], 2, None).await?;
store.delete(&["doc-1".to_string(), "doc-2".to_string()]).await?;
```

The runnable Rust implementation is in `wesichain-weaviate/examples/rag_integration.rs`.

## Verification Commands

Run these from the workspace root:

```bash
cargo test -p wesichain-weaviate migration_parity -- --nocapture
cargo check -p wesichain-weaviate --examples
cargo test -p wesichain-weaviate --tests -- --nocapture
```

Optional local run against Weaviate:

```bash
WEAVIATE_URL=http://127.0.0.1:8080 \
WEAVIATE_CLASS=WesichainDocs \
cargo run -p wesichain-weaviate --example rag_integration
```
