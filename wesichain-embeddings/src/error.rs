use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbeddingProviderError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}
