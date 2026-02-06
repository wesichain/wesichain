use thiserror::Error;
use wesichain_core::StoreError;

#[derive(Debug, Error)]
pub enum PineconeStoreError {
    #[error("invalid configuration: {0}")]
    Config(String),
    #[error("transport failure: {0}")]
    Transport(String),
    #[error(
        "pinecone api error {status}: {message} (retry_after={retry_after_seconds:?}, namespace={namespace:?}, batch_size={batch_size:?})"
    )]
    Api {
        status: u16,
        message: String,
        retry_after_seconds: Option<u64>,
        namespace: Option<String>,
        batch_size: Option<usize>,
    },
    #[error("malformed response: {0}")]
    Malformed(String),
    #[error("metadata reconstruction failed: missing or non-string text key '{text_key}'")]
    MissingTextKey { text_key: String },
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("batch mismatch: docs={docs}, embeddings={embeddings}")]
    BatchMismatch { docs: usize, embeddings: usize },
}

impl From<PineconeStoreError> for StoreError {
    fn from(value: PineconeStoreError) -> Self {
        match value {
            PineconeStoreError::DimensionMismatch { expected, got } => {
                StoreError::DimensionMismatch { expected, got }
            }
            other => StoreError::Internal(Box::new(other)),
        }
    }
}
