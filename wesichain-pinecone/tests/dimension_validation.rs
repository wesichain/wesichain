use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_core::{Embedding, EmbeddingError};
use wesichain_pinecone::PineconeVectorStore;

#[derive(Clone)]
struct Dim2Embedding;

#[async_trait::async_trait]
impl Embedding for Dim2Embedding {
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
async fn dimension_check_does_not_fail_build_when_mismatch() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/describe_index_stats"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"dimension": 3})))
        .mount(&server)
        .await;

    let result = PineconeVectorStore::builder(Dim2Embedding)
        .base_url(server.uri())
        .api_key("key")
        .validate_dimension(true)
        .build()
        .await;

    assert!(result.is_ok());

    let requests = server.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .any(|req| req.url.path() == "/describe_index_stats")
    );
}
