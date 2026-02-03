#[cfg(feature = "ollama")]
mod ollama_tests {
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use wesichain_core::{Embedding, EmbeddingError};
    use wesichain_embeddings::OllamaEmbedding;

    #[tokio::test]
    async fn ollama_embedding_maps_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "embedding": [0.4, 0.5]
            })))
            .mount(&server)
            .await;

        let embedder = OllamaEmbedding::new(server.uri(), "nomic-embed-text".to_string(), 2);
        let out = embedder.embed("hello").await.unwrap();
        assert_eq!(out, vec![0.4, 0.5]);
    }

    #[tokio::test]
    async fn ollama_embedding_invalid_dimension() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "embedding": [0.4]
            })))
            .mount(&server)
            .await;

        let embedder = OllamaEmbedding::new(server.uri(), "nomic-embed-text".to_string(), 2);
        let err = embedder.embed("hello").await.unwrap_err();
        assert!(matches!(err, EmbeddingError::InvalidResponse(_)));
    }
}
