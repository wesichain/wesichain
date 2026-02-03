use async_trait::async_trait;

use wesichain_core::{Embedding, EmbeddingError};

struct TestEmbedding;

#[async_trait]
impl Embedding for TestEmbedding {
    async fn embed_documents(&self, documents: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(vec![vec![0.0]; documents.len()])
    }

    async fn embed_query(&self, _query: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![0.0])
    }
}

#[tokio::test]
async fn embedding_trait_is_object_safe() {
    let embedding: Box<dyn Embedding> = Box::new(TestEmbedding);
    let result = embedding.embed_query("hello").await.unwrap();
    assert_eq!(result, vec![0.0]);
}
