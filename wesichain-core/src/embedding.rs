use async_trait::async_trait;

use crate::EmbeddingError;

#[async_trait]
pub trait Embedding: Send + Sync {
    async fn embed_documents(&self, documents: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    async fn embed_query(&self, query: &str) -> Result<Vec<f32>, EmbeddingError>;
}
