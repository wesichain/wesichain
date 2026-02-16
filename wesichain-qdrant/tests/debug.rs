use wesichain_qdrant::QdrantVectorStore;

#[test]
fn debug_output_redacts_api_key() {
    let store = QdrantVectorStore::builder()
        .base_url("http://localhost:6333")
        .collection("docs")
        .api_key("super-secret-key")
        .build()
        .unwrap();

    let debug_output = format!("{store:?}");

    assert!(!debug_output.contains("super-secret-key"));
    assert!(debug_output.contains("<redacted>"));
}

#[test]
fn builder_debug_output_redacts_api_key() {
    let builder = QdrantVectorStore::builder()
        .base_url("http://localhost:6333")
        .collection("docs")
        .api_key("super-secret-key");

    let debug_output = format!("{builder:?}");

    assert!(!debug_output.contains("super-secret-key"));
    assert!(debug_output.contains("<redacted>"));
}
