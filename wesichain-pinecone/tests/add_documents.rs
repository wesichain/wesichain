use std::collections::HashMap;

use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_core::{Document, Embedding, EmbeddingError};
use wesichain_pinecone::PineconeVectorStore;

#[derive(Clone)]
struct FixedEmbedding;

#[async_trait::async_trait]
impl Embedding for FixedEmbedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![0.1, 0.2, 0.3])
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect())
    }

    fn dimension(&self) -> usize {
        3
    }
}

#[tokio::test]
async fn add_documents_embeds_and_upserts() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/vectors/upsert"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let store = PineconeVectorStore::builder(FixedEmbedding)
        .base_url(server.uri())
        .api_key("key")
        .build()
        .await
        .unwrap();

    let doc = Document {
        id: "doc-1".to_string(),
        content: "hello".to_string(),
        metadata: HashMap::new(),
        embedding: None,
    };

    store.add_documents(vec![doc], None).await.unwrap();
}

#[tokio::test]
async fn add_documents_chunks_large_batches() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/vectors/upsert"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(3)
        .mount(&server)
        .await;

    let store = PineconeVectorStore::builder(FixedEmbedding)
        .base_url(server.uri())
        .api_key("key")
        .max_batch_size(2)
        .build()
        .await
        .unwrap();

    let docs: Vec<Document> = (0..5)
        .map(|i| Document {
            id: format!("doc-{i}"),
            content: format!("hello-{i}"),
            metadata: HashMap::new(),
            embedding: None,
        })
        .collect();

    store.add_documents(docs, None).await.unwrap();
}
