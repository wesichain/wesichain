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
use wesichain_core::Document;
use wesichain_retrieval::{
    InMemoryVectorStore, RecursiveCharacterTextSplitter, HashEmbedder
};
use wesichain_rag::{WesichainRag, RagQueryRequest};

// 1. Prepare documents
let documents = vec![
    Document::new("Rust is a systems programming language..."),
    Document::new("Tokio is an async runtime for Rust..."),
];

// 2. Create text splitter
let splitter = RecursiveCharacterTextSplitter::builder()
    .chunk_size(1000)
    .chunk_overlap(200)
    .build()?;

// 3. Build RAG pipeline
let rag = WesichainRag::builder()
    .with_splitter(splitter)
    .build()?;

// 4. Index documents
rag.add_documents(documents).await?;

// 5. Query
let request = RagQueryRequest {
    query: "What is Wesichain?".to_string(),
    thread_id: None,
};
let response = rag.query(request).await?;
println!("{}", response.answer);
```

## Key Patterns

### Pattern 1: Complete RAG Pipeline

```rust
use wesichain_rag::WesichainRag;

let rag = WesichainRag::builder()
    .with_embedder(my_embedder)
    .with_vector_store(my_store)
    .with_max_retries(3)
    .build()?;

// Index documents
rag.add_documents(docs).await?;

// Query
let response = rag.query(RagQueryRequest {
    query: "Your question".to_string(),
    thread_id: None,
}).await?;
```

### Pattern 2: RAG with Custom Components

```rust
use wesichain_rag::WesichainRag;
use wesichain_graph::InMemoryCheckpointer;

let rag = WesichainRag::builder()
    .with_embedder(HashEmbedder::new(384))
    .with_vector_store(InMemoryVectorStore::new())
    .with_checkpointer(InMemoryCheckpointer::default())
    .with_max_retries(3)
    .build()?;

// Process file directly
rag.process_file(std::path::Path::new("./docs.txt")).await?;
```

### Pattern 3: Direct Similarity Search

```rust
// Search without generating answer
let results = rag.similarity_search("async rust", 5).await?;

for result in results {
    println!("Score: {:.2}, Content: {}",
        result.score,
        result.document.content
    );
}
```

### Pattern 4: Streaming RAG Response

```rust
use wesichain_core::AgentEvent;
use futures::StreamExt;

let mut stream = rag.query_stream(RagQueryRequest {
    query: "Explain async Rust".to_string(),
    thread_id: None,
}).await?;

while let Some(event) = stream.next().await {
    match event? {
        AgentEvent::Thought { content, .. } => println!("🤔 {}", content),
        AgentEvent::Final { content, .. } => println!("✅ {}", content),
        AgentEvent::Error { message, .. } => eprintln!("❌ {}", message),
        _ => {}
    }
}
```

## Golden Rules

1. **Use the builder pattern** - All RAG components use builders (WesichainRag::builder(), RecursiveCharacterTextSplitter::builder())
2. **Chunk size matters** - Use 500-1000 tokens for precise retrieval, 1500-2000 for context-heavy questions; always include 10-20% overlap
3. **Thread IDs for state** - Use consistent thread_ids in RagQueryRequest for multi-turn conversations
4. **Handle empty results** - Always check if retrieval returned documents and provide fallback behavior
5. **Use HashEmbedder for testing** - Fast, deterministic embeddings for tests without external services

## Common Mistakes

- **Calling methods on unbuilt RAG** - Must call .build()? before using WesichainRag
- **Forgetting thread_id** - Without thread_id, each query starts fresh; use consistent IDs for conversation state
- **Wrong embedder dimension** - HashEmbedder::new(384) creates 384-dim vectors; ensure consistency
- **Documents too long** - Always split documents before indexing; use RecursiveCharacterTextSplitter
- **No error handling** - RAG returns Result<T, RagError>; use ? or match for proper handling

## Resources

- Full guide: `docs/skills/rag-pipelines.md`
- Key crates: `wesichain-rag`, `wesichain-retrieval`, `wesichain-core`
- Types: `WesichainRag`, `RagQueryRequest`, `RagQueryResponse`, `AgentEvent`
- Builder methods: `with_embedder`, `with_vector_store`, `with_checkpointer`, `with_splitter`, `with_max_retries`
