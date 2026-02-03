use thiserror::Error;

#[derive(Debug, Error)]
#[error("embedding provider error")]
pub struct EmbeddingProviderError;
