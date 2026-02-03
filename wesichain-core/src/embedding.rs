use async_trait::async_trait;

use crate::EmbeddingError;

#[async_trait]
pub trait Embedding: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    fn dimension(&self) -> usize;
}
