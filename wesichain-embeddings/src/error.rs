use thiserror::Error;
use wesichain_core::EmbeddingError;

#[derive(Debug, Error)]
pub enum EmbeddingProviderError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

impl From<EmbeddingProviderError> for EmbeddingError {
    fn from(error: EmbeddingProviderError) -> Self {
        match error {
            EmbeddingProviderError::InvalidResponse(message) => {
                EmbeddingError::InvalidResponse(message)
            }
            EmbeddingProviderError::Request(message) => EmbeddingError::Provider(message),
        }
    }
}
