---
name: wesichain-embeddings
description: |
  Text embedding providers for Wesichain: OpenAI, Ollama, Google, and Candle.
  Generate vector representations of text for RAG, semantic search, and similarity.
triggers:
  - "embedding"
  - "embed"
  - "text-embedding"
  - "OpenAiEmbedding"
  - "OllamaEmbedding"
  - "GoogleEmbedding"
  - "CandleEmbedding"
  - "wesichain-embeddings"
  - "vector"
  - "embedding model"
---

## When to Use

Use wesichain-embeddings when you need to:
- Convert text into vector representations for semantic similarity
- Build RAG pipelines that require document embeddings
- Compare text similarity using cosine similarity or dot product
- Choose between cloud APIs (OpenAI, Google) vs local inference (Ollama, Candle)

## Quick Start

```rust
use wesichain_core::Embedding;
use wesichain_embeddings::OpenAiEmbedding;

// Create an embedder
let embedder = OpenAiEmbedding::new(
    std::env::var("OPENAI_API_KEY")?,
    "text-embedding-3-small",
    1536,
);

// Single text embedding
let embedding = embedder.embed("Hello world").await?;
println!("Vector length: {}", embedding.len()); // 1536

// Batch embedding for efficiency
let texts = vec![
    "First document".to_string(),
    "Second document".to_string(),
];
let embeddings = embedder.embed_batch(&texts).await?;
```

## Key Patterns

### Pattern 1: OpenAI Embeddings (Cloud API)

Best for: Production use, highest quality, pay-per-use

```rust
use wesichain_embeddings::OpenAiEmbedding;
use wesichain_core::Embedding;

// text-embedding-3-small: 1536 dims, fast, cost-effective
let embedder = OpenAiEmbedding::new(
    std::env::var("OPENAI_API_KEY")?,
    "text-embedding-3-small",
    1536,
);

// text-embedding-3-large: 3072 dims, highest quality
let embedder_large = OpenAiEmbedding::new(
    std::env::var("OPENAI_API_KEY")?,
    "text-embedding-3-large",
    3072,
);

// With custom client for proxy/base URL
use async_openai::{Client, config::OpenAIConfig};
let config = OpenAIConfig::default()
    .with_api_key(api_key)
    .with_api_base("https://proxy.example.com/v1".to_string());
let client = Client::with_config(config);
let embedder = OpenAiEmbedding::with_client(client, "text-embedding-3-small", 1536);
```

### Pattern 2: Ollama Embeddings (Local Server)

Best for: Privacy (data never leaves your machine), cost control, offline use

```rust
use wesichain_embeddings::OllamaEmbedding;
use wesichain_core::Embedding;

// nomic-embed-text: 768 dims, fast, good quality
let embedder = OllamaEmbedding::new(
    "http://localhost:11434".to_string(),
    "nomic-embed-text".to_string(),
    768,
);

// mxbai-embed-large: 1024 dims, higher quality
let embedder = OllamaEmbedding::new(
    "http://localhost:11434".to_string(),
    "mxbai-embed-large".to_string(),
    1024,
);

// Usage is identical to OpenAI
let embedding = embedder.embed("Hello world").await?;
let batch = embedder.embed_batch(&["doc1".to_string(), "doc2".to_string()]).await?;
```

**Prerequisites:**
```bash
# Install Ollama: https://ollama.com
ollama pull nomic-embed-text
ollama serve
```

### Pattern 3: Google Embeddings (Cloud API)

Best for: Google Cloud ecosystem, competitive pricing, task-specific embeddings

```rust
use wesichain_embeddings::GoogleEmbedding;
use wesichain_core::Embedding;

// text-embedding-004: 768 dims
let embedder = GoogleEmbedding::new(
    std::env::var("GOOGLE_API_KEY")?,
    "text-embedding-004",
    768,
);

// With task type for better retrieval performance
let embedder = GoogleEmbedding::new(api_key, "text-embedding-004", 768)
    .with_task_type("RETRIEVAL_DOCUMENT");  // or "RETRIEVAL_QUERY"

// Custom base URL for Vertex AI
let embedder = GoogleEmbedding::new(api_key, "text-embedding-004", 768)
    .with_base_url("https://us-central1-aiplatform.googleapis.com");
```

### Pattern 4: Candle Embeddings (Fully Local)

Best for: No server required, Rust-native inference, maximum privacy

```rust
use wesichain_embeddings::CandleEmbedding;
use wesichain_core::Embedding;

// Note: CandleEmbedding is a placeholder in current implementation
// Full implementation loads models directly in Rust
let embedder = CandleEmbedding::new(model_path)?;
let embedding = embedder.embed("Hello world").await?;
```

### Pattern 5: Batching for Performance

```rust
use wesichain_core::Embedding;

// Always use batch embedding for multiple texts
// - OpenAI/Google: Single HTTP request for all texts
// - Ollama: Parallel requests (Ollama handles one at a time)

let documents: Vec<String> = load_documents();

// Process in chunks to avoid rate limits
const BATCH_SIZE: usize = 100;
let mut all_embeddings = Vec::new();

for chunk in documents.chunks(BATCH_SIZE) {
    let embeddings = embedder.embed_batch(chunk).await?;
    all_embeddings.extend(embeddings);
}
```

### Pattern 6: Choosing Dimensions and Models

```rust
// OpenAI - Recommended
// text-embedding-3-small: 1536 dims, ~$0.02/1M tokens, fast, good quality
// text-embedding-ada-002: 1536 dims, legacy, use 3-small instead
// text-embedding-3-large: 3072 dims, ~$0.13/1M tokens, highest quality

// Ollama - Free, local
// nomic-embed-text: 768 dims, fast, multilingual
// mxbai-embed-large: 1024 dims, higher quality
// snowflake-arctic-embed: 1024 dims, optimized for short texts

// Google
// text-embedding-004: 768 dims, competitive pricing
// text-embedding-005: 768 dims, latest version

// Dimension selection:
// - Higher = more accurate, more storage, slower queries
// - 384-768: Good for prototyping, smaller datasets
// - 1024-1536: Standard for production
// - 3072: Maximum accuracy, larger storage cost
```

### Pattern 7: Generic Embedding Trait

```rust
use wesichain_core::Embedding;
use std::sync::Arc;

// Write code that works with any embedder
async fn index_documents<E: Embedding>(
    embedder: &E,
    documents: &[String],
) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    embedder.embed_batch(documents).await
}

// Or use Arc<dyn> for runtime polymorphism
async fn index_documents_dyn(
    embedder: Arc<dyn Embedding>,
    documents: &[String],
) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    embedder.embed_batch(documents).await
}
```

## Golden Rules

1. **Always specify dimension** - Must match the model's output dimension exactly
2. **Use batch embedding** - embed_batch() is much faster than multiple embed() calls
3. **Match dimensions to your vector store** - Qdrant/Chroma index dimension must equal embedder.dimension()
4. **Use Arc<dyn Embedding> for polymorphism** - Required when passing embedders across async boundaries
5. **Set environment variables outside code** - Never hardcode API keys
6. **Use text-embedding-3-small for most use cases** - Best balance of cost/quality

## Common Mistakes

- **Wrong dimension** - Declaring 1536 for a 768-dim model causes `InvalidResponse` errors
- **Not enabling feature flags** - Cargo.toml needs `features = ["openai"]` etc.
- **Using embed() in a loop** - Use embed_batch() instead for multiple texts
- **Mismatched dimensions in vector store** - Creating Qdrant collection with 1536 dims but using 768-dim model
- **Dimension mismatch in batch** - All texts must be embedded to same dimension
- **Forgetting to start Ollama** - OllamaEmbedding requires `ollama serve` running
- **Wrong model name** - Use "nomic-embed-text", not "nomic-embed-text:v1.5"
- **Ignoring rate limits** - Implement backoff when hitting OpenAI/Google quotas

## Resources

- Crate: `wesichain-embeddings`
- Feature flags: `openai`, `ollama`, `google`, `candle`
- Core trait: `wesichain_core::Embedding`
- Key methods: `embed()`, `embed_batch()`, `dimension()`
- OpenAI models: https://platform.openai.com/docs/guides/embeddings
- Ollama models: https://ollama.com/library (search "embed")
- Google models: https://ai.google.dev/gemini-api/docs/embeddings
- Error type: `wesichain_embeddings::EmbeddingProviderError`
