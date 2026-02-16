use thiserror::Error;
use wesichain_core::StoreError;

#[derive(Debug, Error)]
pub enum QdrantStoreError {
    #[error("invalid configuration: base_url is required")]
    MissingBaseUrl,
    #[error("invalid configuration: base_url cannot be empty")]
    EmptyBaseUrl,
    #[error("invalid configuration: collection is required")]
    MissingCollection,
    #[error("invalid configuration: collection cannot be empty")]
    EmptyCollection,
    #[error("invalid document id: {0}")]
    InvalidDocumentId(String),
    #[error("document '{id}' is missing embedding")]
    MissingEmbedding { id: String },
    #[error("qdrant point '{point_id}' is missing content payload '__wesichain_content'")]
    MissingContentPayload { point_id: String },
    #[error(
        "qdrant point '{point_id}' has invalid content payload type: expected {expected}, got {actual}"
    )]
    InvalidContentPayloadType {
        point_id: String,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("metadata filters are not supported yet")]
    UnsupportedFilter,
    #[error("unsupported metadata filter value for key '{key}': {reason}")]
    UnsupportedFilterValue { key: String, reason: String },
    #[error("qdrant request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("collection '{collection}' not found: {message}")]
    CollectionNotFound { collection: String, message: String },
    #[error("qdrant returned HTTP {status}: {message}")]
    HttpStatus { status: u16, message: String },
    #[error("invalid qdrant response: {message}")]
    InvalidResponse { message: String },
}

impl From<QdrantStoreError> for StoreError {
    fn from(value: QdrantStoreError) -> Self {
        match value {
            QdrantStoreError::InvalidDocumentId(id) => StoreError::InvalidId(id),
            other => StoreError::Internal(Box::new(other)),
        }
    }
}
