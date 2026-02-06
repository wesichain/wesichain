# wesichain-pinecone

Pinecone vector store integration for Wesichain with LangChain-like ergonomics and Rust-native APIs.

## Features

- External embedding provider injection (`E: Embedding`)
- Data-plane operations: upsert, query, delete
- Typed metadata filters with raw JSON fallback
- LangChain-style methods:
  - `add_documents`
  - `similarity_search`
  - `similarity_search_with_score`
  - `delete`

## Environment Variables

- `PINECONE_API_KEY` (required)
- `PINECONE_BASE_URL` (required, full HTTPS endpoint)
- `PINECONE_NAMESPACE` (optional)

## Quickstart

```rust
use std::collections::HashMap;

use wesichain_core::{Document, Embedding, EmbeddingError};
use wesichain_pinecone::PineconeVectorStore;

#[derive(Clone)]
struct DummyEmbedding;

#[async_trait::async_trait]
impl Embedding for DummyEmbedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![0.1, 0.2, 0.3])
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect())
    }

    fn dimension(&self) -> usize {
        3
    }
}

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let store = PineconeVectorStore::builder(DummyEmbedding)
    .base_url(std::env::var("PINECONE_BASE_URL")?)
    .api_key(std::env::var("PINECONE_API_KEY")?)
    .build()
    .await?;

let docs = vec![Document {
    id: "doc-1".to_string(),
    content: "Wesichain is Rust-native".to_string(),
    metadata: HashMap::new(),
    embedding: None,
}];

store.add_documents(docs, None).await?;
let results = store.similarity_search("what is wesichain?", 3, None).await?;
println!("{}", results.len());
# Ok(())
# }
```

## Migration Table (LangChain -> Wesichain)

| Python (LangChain) | Rust (Wesichain) | Notes |
|---|---|---|
| `PineconeVectorStore(index_name=..., embedding=...)` | `PineconeVectorStore::builder(embedder).base_url(...).api_key(...).build().await?` | Uses full Pinecone index URL |
| `add_documents(docs)` | `add_documents(docs, None).await?` | Optional explicit IDs supported |
| `similarity_search(query, k=5, filter=...)` | `similarity_search(query, 5, Some(filter)).await?` | Typed `MetadataFilter` |
| `similarity_search_with_score(...)` | `similarity_search_with_score(...).await?` | Returns `Vec<(Document, f32)>` |
| `delete(ids=[...])` | `delete(&ids).await?` | Also supports `delete_vec(ids)` |
| `GoogleGenerativeAIEmbeddings(model="models/embedding-001")` | `wesichain_embeddings::GoogleEmbedding::new("<api-key>", "models/embedding-001")?` | External embedder injection preserved |

## Tracing

Set `RUST_LOG` to inspect request flow:

```bash
RUST_LOG=wesichain_pinecone=debug
```
