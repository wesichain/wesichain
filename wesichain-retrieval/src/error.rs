use thiserror::Error;
use wesichain_core::{EmbeddingError, StoreError};

#[derive(Debug, Error)]
pub enum RetrievalError {
    #[error("invalid document id: {0}")]
    InvalidId(String),
    #[error("embedding error: {0}")]
    Embedding(#[from] EmbeddingError),
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}

pub type RetrievalResult<T> = Result<T, RetrievalError>;
