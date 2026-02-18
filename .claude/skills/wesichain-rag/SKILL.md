---
name: wesichain-rag
description: |
  Build production-ready Retrieval-Augmented Generation (RAG) pipelines with
  document processing, embeddings, vector stores, and context-aware LLM responses.
  Use for document Q&A, knowledge bases, and semantic search applications.
triggers:
  - "rag"
  - "retrieval"
  - "vector store"
  - "embeddings"
  - "document"
  - "chunk"
  - "semantic search"
  - "wesichain-rag"
---

## When to Use

Use wesichain-rag when you need to:
- Build document Q&A systems that retrieve relevant context before generating answers
- Create knowledge bases from unstructured text documents
- Implement semantic search over document collections
- Combine vector similarity with LLM generation for accurate, grounded responses

## Quick Start

```rust
use wesichain_core::{Runnable, Document};
use wesichain_llm::{OllamaClient, OllamaEmbeddings};
use wesichain_text_splitter::RecursiveCharacterSplitter;
use wesichain_vector_store::InMemoryVectorStore;
use wesichain_rag::RagPipeline;

// 1. Prepare and chunk documents
let documents = vec![
    Document::new("Rust is a systems programming language..."),
    Document::new("Tokio is an async runtime for Rust..."),
];
let splitter = RecursiveCharacterSplitter::new()
    .chunk_size(500)
    .chunk_overlap(50);
let chunks = splitter.split_documents(&documents);

// 2. Initialize embeddings and vector store
let embeddings = OllamaEmbeddings::new("http://localhost:11434", "nomic-embed-text");
let mut vector_store = InMemoryVectorStore::new(embeddings.embedding_dimension());
vector_store.add_documents(&chunks, &embeddings).await?;

// 3. Build RAG pipeline
let rag = RagPipeline::new()
    .with_retriever(vector_store.as_retriever().top_k(3))
    .with_llm(OllamaClient::new("http://localhost:11434", "llama3.2"))
    .with_template("Context: {context}\n\nQuestion: {question}\n\nAnswer:");

// 4. Query
let answer = rag.invoke("What is Wesichain?").await?;
```

## Key Patterns

### Pattern 1: Complete RAG Pipeline

```rust
use wesichain_rag::RagPipeline;

let rag = RagPipeline::new()
    .with_retriever(retriever)
    .with_llm(llm)
    .with_template("Context: {context}\n\nQuestion: {question}\n\nAnswer:");

let answer = rag.invoke("Your question here").await?;
```

### Pattern 2: RAG with Qdrant Vector Store

```rust
use wesichain_vector_store::QdrantVectorStore;
use qdrant_client::QdrantClient;

let qdrant_client = QdrantClient::from_url("http://localhost:6334").build()?;
let vector_store = QdrantVectorStore::new(
    qdrant_client,
    "my_collection",
    embeddings.embedding_dimension(),
)
.with_distance_metric("cosine")
.create_collection_if_missing()
.await?;

vector_store.add_documents(&chunks, &embeddings).await?;
let retriever = vector_store.as_retriever().top_k(5);
```

### Pattern 3: Custom Retriever with Filters

```rust
use wesichain_vector_store::SearchFilter;

// Create documents with metadata
let docs = vec![
    Document::new("Rust memory safety...").with_metadata("category", "language"),
    Document::new("Tokio async runtime...").with_metadata("category", "runtime"),
];

// Build and apply filter
let filter = SearchFilter::new()
    .must_eq("category", "runtime")
    .must_not_eq("status", "deprecated");

let retriever = vector_store
    .as_retriever()
    .top_k(3)
    .with_filter(filter);
```

### Pattern 4: Streaming RAG Response

```rust
use wesichain_core::Runnable;
use futures::StreamExt;

let rag = RagPipeline::new()
    .with_retriever(retriever)
    .with_llm(llm)
    .with_template("Context: {context}\n\nQuestion: {question}");

let mut stream = rag.stream("Explain async Rust").await?;
while let Some(chunk) = stream.next().await {
    match chunk {
        Ok(text) => print!("{}", text),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Golden Rules

1. **Chunk size matters** - Use 200-500 tokens for precise retrieval, 1000-2000 for context-heavy questions; always include 10-20% overlap
2. **Never mix embedding models** - Use the same model for indexing and querying; store the model name with your index
3. **Handle empty context gracefully** - Always check if retrieval returned documents and provide fallback behavior
4. **Use metadata for filtering** - Tag documents with source, category, date for targeted retrieval
5. **Consider hybrid search** - Combine vector similarity with keyword search for better results on specific terms

## Common Mistakes

- **Embedding dimension mismatch** - Using different models for indexing (768d) vs querying (384d); always use the same model
- **Documents too long** - Exceeding token limits; always split documents before embedding
- **No fallback for empty retrieval** - Failing when no relevant documents found; always handle this case
- **Ignoring chunk overlap** - Losing context at chunk boundaries; always include 10-20% overlap
- **Not validating retrieved context** - Blindly trusting retrieved chunks; validate relevance before using

## Resources

- Full guide: `/Users/bene/Documents/bene/python/rechain/wesichain/.worktrees/ai-skills-docs/docs/skills/rag-pipelines.md`
- Key crates: `wesichain-rag`, `wesichain-vector-store`, `wesichain-text-splitter`
- Vector stores: InMemoryVectorStore, QdrantVectorStore
- Embeddings: OllamaEmbeddings, OpenAIEmbeddings
