#[cfg(feature = "google")]
mod google_tests {
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use wesichain_core::{Embedding, EmbeddingError};
    use wesichain_embeddings::GoogleEmbedding;

    #[tokio::test]
    async fn google_embedding_maps_single_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1beta/models/text-embedding-004:embedContent"))
            .and(query_param("key", "test-key"))
            .and(header("content-type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "embedding": {
                    "values": [0.1, 0.2, 0.3]
                }
            })))
            .mount(&server)
            .await;

        let embedder =
            GoogleEmbedding::new("test-key", "text-embedding-004", 3).with_base_url(server.uri());

        let out = embedder.embed("hello").await.unwrap();
        assert_eq!(out, vec![0.1, 0.2, 0.3]);
    }

    #[tokio::test]
    async fn google_embedding_maps_batch_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1beta/models/text-embedding-004:batchEmbedContents"))
            .and(query_param("key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "embeddings": [
                    {"values": [0.1, 0.2, 0.3]},
                    {"values": [0.4, 0.5, 0.6]}
                ]
            })))
            .mount(&server)
            .await;

        let embedder =
            GoogleEmbedding::new("test-key", "text-embedding-004", 3).with_base_url(server.uri());
        let inputs = vec!["hello".to_string(), "world".to_string()];

        let out = embedder.embed_batch(&inputs).await.unwrap();
        assert_eq!(out, vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]]);
    }

    #[tokio::test]
    async fn google_embedding_batch_count_mismatch_returns_invalid_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1beta/models/text-embedding-004:batchEmbedContents"))
            .and(query_param("key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "embeddings": [
                    {"values": [0.1, 0.2, 0.3]}
                ]
            })))
            .mount(&server)
            .await;

        let embedder =
            GoogleEmbedding::new("test-key", "text-embedding-004", 3).with_base_url(server.uri());
        let inputs = vec!["hello".to_string(), "world".to_string()];

        let err = embedder.embed_batch(&inputs).await.unwrap_err();
        assert!(matches!(err, EmbeddingError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn google_embedding_invalid_dimension_returns_invalid_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1beta/models/text-embedding-004:embedContent"))
            .and(query_param("key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "embedding": {
                    "values": [0.1, 0.2]
                }
            })))
            .mount(&server)
            .await;

        let embedder =
            GoogleEmbedding::new("test-key", "text-embedding-004", 3).with_base_url(server.uri());

        let err = embedder.embed("hello").await.unwrap_err();
        assert!(matches!(err, EmbeddingError::InvalidResponse(_)));
    }
}
