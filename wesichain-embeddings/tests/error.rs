use wesichain_core::EmbeddingError;
use wesichain_embeddings::EmbeddingProviderError;

#[test]
fn embedding_provider_error_maps_to_embedding_error() {
    let invalid_response: EmbeddingError =
        EmbeddingProviderError::InvalidResponse("bad payload".to_string()).into();
    assert!(matches!(
        &invalid_response,
        EmbeddingError::InvalidResponse(message) if message == "bad payload"
    ));
    assert_eq!(
        format!("{invalid_response}"),
        "Embedding invalid response: bad payload"
    );

    let request: EmbeddingError =
        EmbeddingProviderError::Request("upstream timeout".to_string()).into();
    assert!(matches!(
        &request,
        EmbeddingError::Provider(message) if message == "upstream timeout"
    ));
    assert_eq!(
        format!("{request}"),
        "Embedding provider error: upstream timeout"
    );
}
