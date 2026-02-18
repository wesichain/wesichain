# Wesichain RAG Pipelines

Build production-ready Retrieval-Augmented Generation (RAG) systems with efficient document processing, embedding generation, and context-aware LLM responses.

## Quick Reference

### Key Crates

```rust
use wesichain_core::{Runnable, Chain, Document};
use wesichain_prompt::PromptTemplate;
use wesichain_llm::{OllamaClient, OllamaEmbeddings};
use wesichain_text_splitter::{TextSplitter, RecursiveCharacterSplitter};
use wesichain_vector_store::{InMemoryVectorStore, QdrantVectorStore, Retriever};
use wesichain_rag::RagPipeline;
```

### RAG Flow

```
Documents → TextSplitter → Embeddings → VectorStore → Retriever → LLM → Answer
     ↓            ↓             ↓              ↓            ↓        ↓
  Raw Text    Chunks      Vectors      Indexed     Context   Response
```

### Type Signatures

| Component | Type | Purpose |
|-----------|------|---------|
| TextSplitter | `RecursiveCharacterSplitter` | Split documents into chunks |
| Embeddings | `OllamaEmbeddings` | Generate vector embeddings |
| VectorStore | `InMemoryVectorStore` / `QdrantVectorStore` | Store and search vectors |
| Retriever | `Retriever` | Fetch relevant documents |
| RAG Pipeline | `RagPipeline` | End-to-end RAG workflow |

## Code Patterns

### Pattern 1: Complete RAG Pipeline (Ollama)

Full end-to-end RAG system using local Ollama embeddings and LLM.

```rust
use wesichain_core::{Runnable, Document};
use wesichain_llm::{OllamaClient, OllamaEmbeddings};
use wesichain_text_splitter::RecursiveCharacterSplitter;
use wesichain_vector_store::InMemoryVectorStore;
use wesichain_rag::RagPipeline;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Prepare documents
    let documents = vec![
        Document::new("Rust is a systems programming language..."),
        Document::new("Tokio is an asynchronous runtime for Rust..."),
        Document::new("Wesichain provides high-performance LLM chains..."),
    ];

    // 2. Split documents into chunks
    let splitter = RecursiveCharacterSplitter::new()
        .chunk_size(500)
        .chunk_overlap(50);
    let chunks = splitter.split_documents(&documents);

    // 3. Initialize embeddings
    let embeddings = OllamaEmbeddings::new("http://localhost:11434", "nomic-embed-text");

    // 4. Create vector store and add documents
    let mut vector_store = InMemoryVectorStore::new(embeddings.embedding_dimension());
    vector_store.add_documents(&chunks, &embeddings).await?;

    // 5. Create retriever
    let retriever = vector_store.as_retriever().top_k(3);

    // 6. Initialize LLM
    let llm = OllamaClient::new("http://localhost:11434", "llama3.2");

    // 7. Build RAG pipeline
    let rag = RagPipeline::new()
        .with_retriever(retriever)
        .with_llm(llm)
        .with_template("Context: {context}\n\nQuestion: {question}\n\nAnswer:");

    // 8. Query
    let answer = rag.invoke("What is Wesichain?").await?;
    println!("{}", answer);

    Ok(())
}
```

### Pattern 2: RAG with Qdrant Vector Store

Use Qdrant for persistent, scalable vector storage.

```rust
use wesichain_llm::OllamaEmbeddings;
use wesichain_vector_store::QdrantVectorStore;
use qdrant_client::QdrantClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to Qdrant
    let qdrant_client = QdrantClient::from_url("http://localhost:6334").build()?;
    
    let embeddings = OllamaEmbeddings::new("http://localhost:11434", "nomic-embed-text");
    
    // Create or connect to collection
    let vector_store = QdrantVectorStore::new(
        qdrant_client,
        "my_collection",
        embeddings.embedding_dimension(),
    )
    .with_distance_metric("cosine")
    .create_collection_if_missing()
    .await?;

    // Add documents
    vector_store.add_documents(&chunks, &embeddings).await?;

    // Use in RAG pipeline
    let retriever = vector_store.as_retriever().top_k(5);
    
    Ok(())
}
```

### Pattern 3: Custom Retriever with Filters

Implement custom retrieval logic with metadata filtering.

```rust
use wesichain_core::Document;
use wesichain_vector_store::{Retriever, SearchFilter};

// Create documents with metadata
let docs = vec![
    Document::new("Rust memory safety...").with_metadata("category", "language"),
    Document::new("Tokio async runtime...").with_metadata("category", "runtime"),
    Document::new("Serde serialization...").with_metadata("category", "library"),
];

// Build filter
let filter = SearchFilter::new()
    .must_eq("category", "runtime")
    .must_not_eq("status", "deprecated");

// Apply to retriever
let retriever = vector_store
    .as_retriever()
    .top_k(3)
    .with_filter(filter);

// Custom retriever implementation
struct CategoryRetriever {
    store: InMemoryVectorStore,
    default_category: String,
}

impl Retriever for CategoryRetriever {
    async fn retrieve(
        &self, 
        query: &str, 
        embeddings: &dyn Embeddings
    ) -> Result<Vec<Document>> {
        let filter = SearchFilter::new()
            .must_eq("category", &self.default_category);
        
        self.store
            .search(query, embeddings, 5)
            .with_filter(filter)
            .await
    }
}
```

### Pattern 4: Streaming RAG Response

Stream RAG responses for real-time user experience.

```rust
use wesichain_core::Runnable;
use futures::StreamExt;

let rag = RagPipeline::new()
    .with_retriever(retriever)
    .with_llm(llm)
    .with_template("Context: {context}\n\nQuestion: {question}");

// Stream the response
let mut stream = rag.stream("Explain async Rust").await?;

while let Some(chunk) = stream.next().await {
    match chunk {
        Ok(text) => print!("{}", text),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Vibe Coding Prompts

### Prompt 1: Basic Document Q&A

"Create a RAG pipeline that answers questions about a set of text documents. Use Ollama for embeddings and LLM, split documents into 500-character chunks with 50-character overlap, and retrieve the top 3 most relevant chunks to answer questions."

### Prompt 2: Wikipedia RAG

"Build a RAG system that fetches Wikipedia articles on a topic, indexes them in Qdrant, and answers questions using that knowledge. Include error handling for when no relevant context is found."

### Prompt 3: Code Documentation Search

"Create a code documentation RAG system that indexes Rust crate documentation. Use metadata filters to search by module name, and return code examples along with explanations."

### Prompt 4: Multi-Query RAG

"Implement a multi-query RAG that generates 3 variations of the user's question, retrieves documents for each variation, and combines the results before generating the final answer to improve recall."

## Common Errors

### Error: embedding dimension mismatch

```
Error: Vector dimension mismatch: expected 768, got 384
```

**Cause**: Using different embedding models for indexing and querying.

**Fix**: Ensure the same embedding model is used consistently:

```rust
// Use a constant or configuration
const EMBEDDING_MODEL: &str = "nomic-embed-text";
let embeddings = OllamaEmbeddings::new(url, EMBEDDING_MODEL);
```

### Error: document too long for embedding

```
Error: Document exceeds maximum token limit (8192)
```

**Fix**: Adjust chunk size or use a larger context embedding model:

```rust
let splitter = RecursiveCharacterSplitter::new()
    .chunk_size(2000)  // Reduce chunk size
    .chunk_overlap(200);
```

### Error: Qdrant collection not found

```
Error: Collection 'my_collection' not found
```

**Fix**: Create collection before use:

```rust
let vector_store = QdrantVectorStore::new(client, "my_collection", dim)
    .create_collection_if_missing()
    .await?;
```

### Error: empty context retrieved

```
Error: No relevant documents found for query
```

**Fix**: Add fallback behavior or lower similarity threshold:

```rust
let retriever = vector_store
    .as_retriever()
    .top_k(3)
    .with_score_threshold(0.5);  // Lower threshold

// Or handle empty results
let docs = retriever.retrieve(query, &embeddings).await?;
if docs.is_empty() {
    return Ok("I don't have enough information to answer that.".to_string());
}
```

### Error: cannot move out of borrowed content

```
Error: cannot move out of `self.embeddings` which is behind a shared reference
```

**Fix**: Use `Arc` for shared ownership or clone:

```rust
use std::sync::Arc;

struct MyRag {
    embeddings: Arc<OllamaEmbeddings>,  // Use Arc
}

// Or clone if cheap
let embeddings_clone = self.embeddings.clone();
```

## Best Practices

1. **Chunk Size Strategy**: Use smaller chunks (200-500 tokens) for precise retrieval, larger chunks (1000-2000) for context-heavy questions. Always include overlap (10-20%) to preserve context across chunk boundaries.

2. **Embedding Consistency**: Never mix embedding models between indexing and querying. Store the embedding model name with your index to prevent mismatches.

3. **Hybrid Search**: Combine vector similarity with keyword search for better results on specific terms:
   ```rust
   let retriever = vector_store
       .as_hybrid_retriever()
       .vector_weight(0.7)
       .keyword_weight(0.3);
   ```

4. **Re-ranking**: Apply a cross-encoder re-ranker to improve retrieval quality:
   ```rust
   let retriever = vector_store
       .as_retriever()
       .top_k(20)           // Retrieve more initially
       .rerank_top_k(5);    // Re-rank and keep best 5
   ```

5. **Caching Embeddings**: Cache embeddings for frequently accessed documents to avoid redundant API calls:
   ```rust
   use wesichain_vector_store::CachedEmbeddings;
   
   let cached_embeddings = CachedEmbeddings::new(embeddings)
       .with_cache_dir("./embedding_cache");
   ```

## See Also

- [Agent Workflows](./agent-workflows.md) - Build agents that use RAG for tool reasoning
- [Prompt Templates](./prompt-templates.md) - Craft effective RAG prompts
- [Vector Stores](../components/vector-stores.md) - Deep dive into vector storage options
- [Embeddings Guide](../components/embeddings.md) - Compare embedding providers
