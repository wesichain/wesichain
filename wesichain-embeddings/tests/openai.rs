#[cfg(feature = "openai")]
mod openai_tests {
    use async_openai::config::OpenAIConfig;
    use async_openai::Client;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use wesichain_core::{Embedding, EmbeddingError};
    use wesichain_embeddings::OpenAiEmbedding;

    #[tokio::test]
    async fn openai_embedding_maps_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [
                    {"embedding": [0.1, 0.2, 0.3], "index": 0, "object": "embedding"}
                ],
                "model": "text-embedding-3-small",
                "object": "list",
                "usage": {"prompt_tokens": 1, "total_tokens": 1}
            })))
            .mount(&server)
            .await;

        let config = OpenAIConfig::new()
            .with_api_key("test-key")
            .with_api_base(format!("{}/v1", server.uri()));
        let client = Client::with_config(config);
        let embedder = OpenAiEmbedding::new(client, "text-embedding-3-small".to_string(), 3);

        let out = embedder.embed("hello").await.unwrap();
        assert_eq!(out, vec![0.1, 0.2, 0.3]);
    }

    #[tokio::test]
    async fn openai_embedding_invalid_dimension() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [
                    {"embedding": [0.1, 0.2], "index": 0, "object": "embedding"}
                ],
                "model": "text-embedding-3-small",
                "object": "list",
                "usage": {"prompt_tokens": 1, "total_tokens": 1}
            })))
            .mount(&server)
            .await;

        let config = OpenAIConfig::new()
            .with_api_key("test-key")
            .with_api_base(format!("{}/v1", server.uri()));
        let client = Client::with_config(config);
        let embedder = OpenAiEmbedding::new(client, "text-embedding-3-small".to_string(), 3);

        let err = embedder.embed("hello").await.unwrap_err();
        assert!(matches!(err, EmbeddingError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn openai_embedding_batch_count_mismatch() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [
                    {"embedding": [0.1, 0.2, 0.3], "index": 0, "object": "embedding"}
                ],
                "model": "text-embedding-3-small",
                "object": "list",
                "usage": {"prompt_tokens": 2, "total_tokens": 2}
            })))
            .mount(&server)
            .await;

        let config = OpenAIConfig::new()
            .with_api_key("test-key")
            .with_api_base(format!("{}/v1", server.uri()));
        let client = Client::with_config(config);
        let embedder = OpenAiEmbedding::new(client, "text-embedding-3-small".to_string(), 3);
        let inputs = vec!["hello".to_string(), "world".to_string()];

        let err = embedder.embed_batch(&inputs).await.unwrap_err();
        assert!(matches!(err, EmbeddingError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn openai_embedding_batch_invalid_dimension() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [
                    {"embedding": [0.1, 0.2, 0.3], "index": 0, "object": "embedding"},
                    {"embedding": [0.4, 0.5], "index": 1, "object": "embedding"}
                ],
                "model": "text-embedding-3-small",
                "object": "list",
                "usage": {"prompt_tokens": 2, "total_tokens": 2}
            })))
            .mount(&server)
            .await;

        let config = OpenAIConfig::new()
            .with_api_key("test-key")
            .with_api_base(format!("{}/v1", server.uri()));
        let client = Client::with_config(config);
        let embedder = OpenAiEmbedding::new(client, "text-embedding-3-small".to_string(), 3);
        let inputs = vec!["hello".to_string(), "world".to_string()];

        let err = embedder.embed_batch(&inputs).await.unwrap_err();
        assert!(matches!(err, EmbeddingError::InvalidResponse(_)));
    }
}
