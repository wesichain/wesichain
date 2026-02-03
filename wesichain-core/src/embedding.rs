use async_trait::async_trait;

use crate::EmbeddingError;

#[async_trait]
pub trait Embedding: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    async fn embed_batch_strs(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError>
    where
        Self: Sized,
    {
        let owned: Vec<String> = texts.iter().map(|text| (*text).to_string()).collect();
        self.embed_batch(&owned).await
    }

    async fn embed_batch_ref<T>(&self, texts: &[T]) -> Result<Vec<Vec<f32>>, EmbeddingError>
    where
        T: AsRef<str> + Sync,
        Self: Sized,
    {
        let owned: Vec<String> = texts.iter().map(|text| text.as_ref().to_string()).collect();
        self.embed_batch(&owned).await
    }

    fn dimension(&self) -> usize;
}
