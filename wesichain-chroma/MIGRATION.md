# Python Chroma -> Wesichain Chroma Migration Guide

## Quick Start

### Python (LangChain)

```python
from langchain_chroma import Chroma
from langchain_google_genai import GoogleGenerativeAIEmbeddings

vectorstore = Chroma(
    collection_name="my_docs",
    embedding_function=GoogleGenerativeAIEmbeddings(model="models/gemini-embedding-001"),
)
```

### Rust (Wesichain)

```rust
use wesichain_chroma::ChromaVectorStore;
use wesichain_rag::WesichainRag;

let store = ChromaVectorStore::new("http://localhost:8000", "my_docs").await?;

let rag = WesichainRag::builder()
    .with_embedder(embedder)
    .with_vector_store(store)
    .build()?;
```

## Key Differences

| Aspect | Python | Rust |
|---|---|---|
| Mode | Embedded (in-process) | Client-server (HTTP) |
| Persistence | Optional (`persist_directory`) | Managed by Chroma server |
| Embeddings | Passed to `Chroma(...)` | Configured on `WesichainRag::builder()` |
| API style | Mostly sync wrappers | `async fn` on store and RAG operations |

## Docker Setup (Ephemeral Mode)

Use this to mimic Python's ephemeral behavior:

```yaml
services:
  chroma:
    image: chromadb/chroma:latest
    ports:
      - "8000:8000"
    environment:
      - IS_PERSISTENT=FALSE
```

Run:

```bash
docker compose -f docker-compose.chroma.yml up -d
```

## Environment Variables

```bash
CHROMA_ENDPOINT=http://localhost:8000
CHROMA_TENANT=default_tenant
CHROMA_DATABASE=default_database
```

## Feature Parity

Supported:

- Add documents with embeddings and metadata
- Similarity search with top-k
- Metadata filtering (`eq`, `in`, `range`, `all`, `any`)
- Delete by id
- Collection creation/get-or-create

Not supported yet:

- Embedded Rust client persistence mode
- Custom distance metric selection through `wesichain-chroma`
- Built-in batch chunking helpers (use caller-side chunking)
