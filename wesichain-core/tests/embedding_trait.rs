use std::sync::Arc;

use async_trait::async_trait;

use wesichain_core::{Embedding, EmbeddingError};

struct TestEmbedding;

#[async_trait]
impl Embedding for TestEmbedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![0.0])
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(vec![vec![0.0]; texts.len()])
    }

    fn dimension(&self) -> usize {
        1
    }
}

fn assert_object_safe(_embedding: Arc<dyn Embedding>) {}

#[test]
fn embedding_trait_is_object_safe() {
    let embedding = Arc::new(TestEmbedding);
    assert_object_safe(embedding);
}
