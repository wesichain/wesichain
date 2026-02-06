use std::collections::HashMap;

use serde_json::json;
use wesichain_core::{Document, Embedding, EmbeddingError, MetadataFilter};
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
        metadata: HashMap::from([("source".to_string(), json!("guide"))]),
        embedding: None,
    }];

    store.add_documents(docs, None).await?;

    let filter = MetadataFilter::Eq("source".to_string(), json!("guide"));
    let results = store
        .similarity_search("How does Wesichain use Pinecone?", 3, Some(filter))
        .await?;
    println!("Retrieved {} filtered docs", results.len());

    let scored_results = store
        .similarity_search_with_score("How does Wesichain use Pinecone?", 10, None)
        .await?;
    let high_confidence = scored_results
        .into_iter()
        .filter(|(_, score)| *score > 0.75)
        .count();
    println!("Retrieved {} high confidence docs", high_confidence);

    Ok(())
}
