// Test that error conversions work seamlessly across crate boundaries
use wesichain_core::{EmbeddingError, StoreError, WesichainError};

#[test]
fn test_embedding_error_conversion() {
    let emb_err = EmbeddingError::InvalidResponse("test error".to_string());
    let wesi_err: WesichainError = emb_err.into();
    assert!(wesi_err.to_string().contains("test error"));
}

#[test]
fn test_store_error_conversion() {
    let store_err = StoreError::InvalidId("doc123".to_string());
    let wesi_err: WesichainError = store_err.into();
    assert!(wesi_err.to_string().contains("doc123"));
}

#[test]
fn test_question_mark_operator() {
    fn returns_embedding_error() -> Result<(), EmbeddingError> {
        Err(EmbeddingError::InvalidResponse("bad embed".to_string()))
    }

    fn uses_question_mark() -> Result<(), WesichainError> {
        returns_embedding_error()?;
        Ok(())
    }

    let result = uses_question_mark();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("bad embed"));
}
