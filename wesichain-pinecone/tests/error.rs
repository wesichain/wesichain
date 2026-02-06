use wesichain_core::StoreError;
use wesichain_pinecone::PineconeStoreError;

#[test]
fn pinecone_error_converts_to_store_error() {
    let err = PineconeStoreError::Config("missing api key".to_string());
    let store_err: StoreError = err.into();
    assert!(format!("{store_err}").contains("Store error"));
}

#[test]
fn api_error_includes_status_and_message() {
    let err = PineconeStoreError::Api {
        status: 429,
        message: "rate limited".to_string(),
        retry_after_seconds: Some(30),
        namespace: Some("prod".to_string()),
        batch_size: Some(50),
    };
    let text = err.to_string();
    assert!(text.contains("429"));
    assert!(text.contains("rate limited"));
}
