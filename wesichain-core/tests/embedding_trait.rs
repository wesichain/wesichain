use std::sync::Arc;

use async_trait::async_trait;

use wesichain_core::{embed_batch_ref_dyn, embed_batch_strs_dyn, Embedding, EmbeddingError};

struct TestEmbedding;

#[async_trait]
impl Embedding for TestEmbedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![0.0])
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts
            .iter()
            .map(|text| vec![0.0; text.len()])
            .collect())
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

#[tokio::test]
async fn embed_batch_strs_helper_works_for_concrete_type() {
    let embedder = TestEmbedding;
    let texts = ["hi", "there"];
    let result = embedder.embed_batch_strs(&texts).await.unwrap();
    assert_eq!(result, vec![vec![0.0; 2], vec![0.0; 5]]);
}

#[tokio::test]
async fn embed_batch_ref_helper_supports_string_inputs() {
    let embedder = TestEmbedding;
    let texts = vec!["ab".to_string(), "c".to_string()];
    let result = embedder.embed_batch_ref(&texts).await.unwrap();
    assert_eq!(result, vec![vec![0.0; 2], vec![0.0; 1]]);
}

#[tokio::test]
async fn embed_batch_ref_helper_supports_str_inputs() {
    let embedder = TestEmbedding;
    let texts = vec!["a", "xyz"];
    let result = embedder.embed_batch_ref(&texts).await.unwrap();
    assert_eq!(result, vec![vec![0.0; 1], vec![0.0; 3]]);
}

#[tokio::test]
async fn embed_batch_helpers_work_for_dyn_embedding() {
    let embedder: Arc<dyn Embedding> = Arc::new(TestEmbedding);
    let texts = ["hey", "all"];
    let result = embed_batch_strs_dyn(embedder.as_ref(), &texts)
        .await
        .unwrap();
    assert_eq!(result, vec![vec![0.0; 3], vec![0.0; 3]]);

    let owned = vec!["one".to_string(), "four".to_string()];
    let result = embed_batch_ref_dyn(embedder.as_ref(), &owned)
        .await
        .unwrap();
    assert_eq!(result, vec![vec![0.0; 3], vec![0.0; 4]]);
}
