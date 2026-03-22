# wesichain-rag

Production-ready RAG pipeline for Wesichain — document loaders, text splitters, retrievers, and LLM synthesis.

Part of the [wesichain](https://github.com/wesichain/wesichain) LLM agent framework.

## Installation

```toml
[dependencies]
wesichain-rag = "0.3"
```

## Quick Start

```rust
use wesichain_rag::{RagPipeline, DocumentLoader, RecursiveSplitter};
use wesichain_retrieval::InMemoryVectorStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load and split documents
    let docs = DocumentLoader::from_file("knowledge.md").load().await?;
    let chunks = RecursiveSplitter::new(512, 64).split(docs);

    // Build a RAG pipeline
    let pipeline = RagPipeline::builder()
        .vector_store(InMemoryVectorStore::new())
        .top_k(5)
        .build();

    pipeline.ingest(chunks).await?;
    let answer = pipeline.query("What is Wesichain?").await?;
    println!("{answer}");
    Ok(())
}
```

## Features

- **Document loaders** — plain text, Markdown, HTML, DOCX, PDF (optional)
- **Text splitters** — recursive character splitter with configurable chunk size and overlap
- **Vector store integration** — works with any `VectorStore` impl (in-memory, Qdrant, Pinecone, Weaviate, Chroma)
- **Reranking** — optional cross-encoder or keyword reranker stage
- **Graph-backed pipeline** — built on `wesichain-graph` for checkpointable, resumable ingestion

## License

Apache-2.0 OR MIT
