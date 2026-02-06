use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_core::{Embedding, EmbeddingError, VectorStore};
use wesichain_pinecone::PineconeVectorStore;

#[derive(Clone)]
struct FixedEmbedding;

#[async_trait::async_trait]
impl Embedding for FixedEmbedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![0.2, 0.3])
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|_| vec![0.2, 0.3]).collect())
    }

    fn dimension(&self) -> usize {
        2
    }
}

#[tokio::test]
async fn vector_store_trait_search_and_delete_work() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "matches": [{"id":"d1","score":0.9,"metadata":{"text":"hello"}}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/vectors/delete"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let store = PineconeVectorStore::builder(FixedEmbedding)
        .base_url(server.uri())
        .api_key("key")
        .build()
        .await
        .unwrap();

    let out = store.search(&[0.2, 0.3], 3, None).await.unwrap();
    assert_eq!(out.len(), 1);

    store.delete(&["d1".to_string()]).await.unwrap();
}
