# Wesichain RAG Pipelines

Build production-ready Retrieval-Augmented Generation (RAG) systems with efficient document processing, embedding generation, and context-aware LLM responses.

## Quick Reference

### Key Crates

```rust
use wesichain_core::{Document, Runnable};
use wesichain_retrieval::{
    InMemoryVectorStore, RecursiveCharacterTextSplitter, Indexer, Retriever
};
use wesichain_rag::{WesichainRag, RagQueryRequest};
```

### RAG Flow

```
Documents → TextSplitter → Indexer → VectorStore → Retriever → LLM → Answer
     ↓            ↓             ↓              ↓            ↓        ↓
  Raw Text    Chunks      Vectors      Indexed     Context   Response
```

### Type Signatures

| Component | Type | Purpose |
|-----------|------|---------|
| TextSplitter | `RecursiveCharacterTextSplitter` | Split documents into chunks |
| Indexer | `Indexer<E, S>` | Index documents into vector store |
| Retriever | `Retriever<E, S>` | Fetch relevant documents |
| VectorStore | `InMemoryVectorStore` | Store and search vectors |
| RAG | `WesichainRag` | End-to-end RAG workflow |

## Code Patterns

### Pattern 1: Complete RAG Pipeline (Ollama)

Full end-to-end RAG system using local Ollama embeddings and LLM.

```rust
use wesichain_core::Document;
use wesichain_retrieval::{
    InMemoryVectorStore, RecursiveCharacterTextSplitter, Indexer, Retriever, HashEmbedder
};
use wesichain_rag::{WesichainRag, RagQueryRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Prepare documents
    let documents = vec![
        Document::new("Rust is a systems programming language..."),
        Document::new("Tokio is an asynchronous runtime for Rust..."),
        Document::new("Wesichain provides high-performance LLM chains..."),
    ];

    // 2. Create text splitter
    let splitter = RecursiveCharacterTextSplitter::builder()
        .chunk_size(1000)
        .chunk_overlap(200)
        .separators(vec!["\n\n", "\n", ". ", " ", ""])
        .build()?;

    // 3. Split documents
    let split_docs = splitter.split_documents(&documents);

    // 4. Build RAG pipeline with default in-memory store
    let rag = WesichainRag::builder()
        .with_splitter(splitter)
        .build()?;

    // 5. Index documents
    rag.add_documents(split_docs).await?;

    // 6. Query
    let request = RagQueryRequest {
        query: "What is Wesichain?".to_string(),
        thread_id: None,
    };
    let response = rag.query(request).await?;

    println!("{}", response.answer);
    Ok(())
}
```

### Pattern 2: RAG with Custom Embedder and Vector Store

Use custom embedding and vector store implementations.

```rust
use std::sync::Arc;
use wesichain_core::{Document, Embedding};
use wesichain_retrieval::{
    InMemoryVectorStore, RecursiveCharacterTextSplitter, Indexer, Retriever
};
use wesichain_rag::WesichainRag;
use wesichain_graph::InMemoryCheckpointer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize components
    let splitter = RecursiveCharacterTextSplitter::builder()
        .chunk_size(1000)
        .chunk_overlap(200)
        .build()?;

    // 2. Use custom embedder (or HashEmbedder for testing)
    let embedder = HashEmbedder::new(384); // 384-dimension embeddings

    // 3. Create vector store
    let vector_store = InMemoryVectorStore::new();

    // 4. Build RAG with explicit components
    let rag = WesichainRag::builder()
        .with_splitter(splitter)
        .with_embedder(embedder)
        .with_vector_store(vector_store)
        .with_checkpointer(InMemoryCheckpointer::default())
        .with_max_retries(3)
        .build()?;

    // 5. Process file
    rag.process_file(std::path::Path::new("./docs.txt")).await?;

    // 6. Query with thread tracking
    let response = rag.query(RagQueryRequest {
        query: "What is this document about?".to_string(),
        thread_id: Some("session-123".to_string()),
    }).await?;

    println!("Answer: {}", response.answer);
    println!("Thread: {}", response.thread_id);

    Ok(())
}
```

### Pattern 3: Custom Retriever with Filters

Access the underlying retriever for custom search logic.

```rust
use wesichain_rag::WesichainRag;
use wesichain_core::SearchResult;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rag = WesichainRag::builder().build()?;

    // Add documents first...
    rag.add_documents(docs).await?;

    // Perform similarity search directly
    let results: Vec<SearchResult> = rag.similarity_search("async rust", 5).await?;

    for result in results {
        println!("Score: {:.2}, Content: {}",
            result.score,
            result.document.content
        );
    }

    // Or with scores
    let results = rag.similarity_search_with_score("query", 10).await?;

    Ok(())
}
```

### Pattern 4: Streaming RAG Response

Stream RAG responses for real-time user experience.

```rust
use wesichain_core::AgentEvent;
use wesichain_rag::{WesichainRag, RagQueryRequest};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rag = WesichainRag::builder().build()?;

    // Add documents...
    rag.add_documents(docs).await?;

    // Stream the response
    let mut stream = rag.query_stream(RagQueryRequest {
        query: "Explain async Rust".to_string(),
        thread_id: None,
    }).await?;

    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::Thought { content, .. } => {
                println!("🤔 Thinking: {}", content);
            }
            AgentEvent::ToolCall { tool_name, input, .. } => {
                println!("🔧 Tool: {}({})", tool_name, input);
            }
            AgentEvent::Observation { output, .. } => {
                println!("📊 Result: {}", output);
            }
            AgentEvent::Final { content, .. } => {
                println!("✅ Answer: {}", content);
            }
            AgentEvent::Error { message, .. } => {
                eprintln!("❌ Error: {}", message);
            }
            _ => {}
        }
    }

    Ok(())
}
```

## Vibe Coding Prompts

### Prompt 1: Basic Document Q&A

"Create a RAG pipeline using WesichainRag that answers questions about text documents. Use RecursiveCharacterTextSplitter with 1000-character chunks and 200-character overlap. Index documents and use the query() method to get answers."

### Prompt 2: File-based RAG

"Build a RAG system using WesichainRag that loads a text file with process_file(), indexes it, and answers questions. Include error handling for file not found and empty results."

### Prompt 3: Streaming RAG

"Implement a streaming RAG with WesichainRag using query_stream(). Handle all AgentEvent variants (Thought, ToolCall, Observation, Final, Error) and print them with emojis for user feedback."

### Prompt 4: RAG with Checkpointing

"Create a RAG system with persistence using with_checkpointer(). Use InMemoryCheckpointer for development. Query with a fixed thread_id to maintain conversation state across multiple queries."

## Common Errors

### Error: retriever not initialized

```
Error: Retriever not initialized - call build() first
```

**Cause**: Using RAG before properly building it.

**Fix**: Ensure you call build() and handle the Result:

```rust
let rag = WesichainRag::builder()
    .with_embedder(embedder)
    .with_vector_store(store)
    .build()?; // Don't forget ?
```

### Error: document too long for embedding

```
Error: Document exceeds maximum token limit
```

**Fix**: Adjust chunk size in the splitter:

```rust
let splitter = RecursiveCharacterTextSplitter::builder()
    .chunk_size(500)  // Reduce chunk size
    .chunk_overlap(50)
    .build()?;
```

### Error: empty context retrieved

```
Runtime error: No relevant documents found
```

**Fix**: Check if documents were indexed and query is relevant:

```rust
// Verify documents are indexed
rag.add_documents(docs).await?;

// Search directly to verify
let results = rag.similarity_search("query", 5).await?;
if results.is_empty() {
    println!("No results - check your documents and query");
}
```

### Error: thread_id not found

```
Error: Checkpoint not found for thread_id: xyz
```

**Fix**: Use a consistent thread_id or generate new one:

```rust
use uuid::Uuid;

let thread_id = existing_thread_id
    .unwrap_or_else(|| Uuid::new_v4().to_string());

let response = rag.query(RagQueryRequest {
    query: "...".to_string(),
    thread_id: Some(thread_id),
}).await?;
```

### Error: cannot move out of borrowed content

```
Error: cannot move out of `self.embeddings` which is behind a shared reference
```

**Fix**: Use Arc for shared ownership:

```rust
use std::sync::Arc;

let embedder = Arc::new(HashEmbedder::new(384));
let rag = WesichainRag::builder()
    .with_embedder((*embedder).clone()) // Clone if needed
    .build()?;
```

## Best Practices

1. **Chunk Size Strategy**: Use smaller chunks (500-1000 tokens) for precise retrieval, larger chunks (1500-2000) for context-heavy questions. Always include overlap (10-20%) to preserve context.

2. **Builder Pattern**: Wesichain uses builder patterns extensively. Chain methods and call build() at the end:
   ```rust
   let rag = WesichainRag::builder()
       .with_embedder(embedder)
       .with_vector_store(store)
       .with_max_retries(3)
       .build()?;
   ```

3. **Thread IDs**: Use consistent thread_ids for multi-turn conversations to maintain state:
   ```rust
   let thread_id = "user-session-123".to_string();
   let response = rag.query(RagQueryRequest {
       query: "...".to_string(),
       thread_id: Some(thread_id.clone()),
   }).await?;
   ```

4. **Error Handling**: RAG operations return `Result<T, RagError>`. Use `?` operator or match on errors:
   ```rust
   match rag.query(request).await {
       Ok(response) => println!("{}", response.answer),
       Err(RagError::Retrieval(e)) => eprintln!("Search failed: {}", e),
       Err(e) => eprintln!("Error: {}", e),
   }
   ```

5. **Testing**: Use `HashEmbedder` for fast, deterministic testing without external services:
   ```rust
   let rag = WesichainRag::builder()
       .with_embedder(HashEmbedder::new(384))
       .build()?;
   ```

## See Also

- [Core Concepts](./core-concepts.md) - Runnable, Chain, Tool traits
- [ReAct Agents](./react-agents.md) - Build agents that use RAG for tool reasoning
- [Examples](https://github.com/wesichain/wesichain/tree/main/examples) - Working RAG examples
- [Crates.io](https://crates.io/crates/wesichain-rag) - wesichain-rag crate
