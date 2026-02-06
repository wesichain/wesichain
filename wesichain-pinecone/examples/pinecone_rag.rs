use std::collections::HashMap;

use wesichain_core::{Document, Embedding, EmbeddingError};
use wesichain_pinecone::PineconeVectorStore;

#[derive(Clone)]
struct DemoEmbedding;

#[async_trait::async_trait]
impl Embedding for DemoEmbedding {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_url = std::env::var("PINECONE_BASE_URL")?;
    let api_key = std::env::var("PINECONE_API_KEY")?;
    let namespace = std::env::var("PINECONE_NAMESPACE").ok();

    let mut builder = PineconeVectorStore::builder(DemoEmbedding)
        .base_url(base_url)
        .api_key(api_key)
        .text_key("text")
        .validate_dimension(true);

    if let Some(ns) = namespace {
        builder = builder.namespace(ns);
    }

    let store = builder.build().await?;

    let docs = vec![Document {
        id: "demo-doc-1".to_string(),
        content: "Wesichain integrates with Pinecone for vector retrieval".to_string(),
        metadata: HashMap::new(),
        embedding: None,
    }];

    store.add_documents(docs, None).await?;

    let results = store
        .similarity_search("How does Wesichain use Pinecone?", 3, None)
        .await?;
    println!("Retrieved {} docs", results.len());

    Ok(())
}
