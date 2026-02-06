use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_core::{Embedding, EmbeddingError, MetadataFilter};
use wesichain_pinecone::PineconeVectorStore;

#[derive(Clone)]
struct FixedEmbedding;

#[async_trait::async_trait]
impl Embedding for FixedEmbedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![0.9, 0.1])
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|_| vec![0.9, 0.1]).collect())
    }

    fn dimension(&self) -> usize {
        2
    }
}

#[tokio::test]
async fn similarity_search_returns_documents() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "matches": [
                {
                    "id": "doc-1",
                    "score": 0.88,
                    "metadata": {"text": "hello", "source": "tweet"}
                }
            ]
        })))
        .mount(&server)
        .await;

    let store = PineconeVectorStore::builder(FixedEmbedding)
        .base_url(server.uri())
        .api_key("key")
        .build()
        .await
        .unwrap();

    let docs = store
        .similarity_search(
            "query",
            3,
            Some(MetadataFilter::Eq("source".to_string(), json!("tweet"))),
        )
        .await
        .unwrap();

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].content, "hello");
}

#[tokio::test]
async fn similarity_search_with_score_returns_score() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "matches": [{"id": "doc-1", "score": 0.66, "metadata": {"text": "hello"}}]
        })))
        .mount(&server)
        .await;

    let store = PineconeVectorStore::builder(FixedEmbedding)
        .base_url(server.uri())
        .api_key("key")
        .build()
        .await
        .unwrap();

    let out = store
        .similarity_search_with_score("query", 2, None)
        .await
        .unwrap();
    assert_eq!(out[0].1, 0.66);
}
