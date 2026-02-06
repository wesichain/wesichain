use wesichain_core::{Embedding, EmbeddingError};
use wesichain_pinecone::PineconeVectorStore;

#[derive(Clone)]
struct DummyEmbedding;

#[async_trait::async_trait]
impl Embedding for DummyEmbedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![0.1, 0.2])
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|_| vec![0.1, 0.2]).collect())
    }

    fn dimension(&self) -> usize {
        2
    }
}

#[tokio::test]
async fn builder_sets_default_text_key() {
    let store = PineconeVectorStore::builder(DummyEmbedding)
        .base_url("https://example.test")
        .api_key("key")
        .build()
        .await
        .unwrap();
    assert_eq!(store.text_key(), "text");
}
