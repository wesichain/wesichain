use thiserror::Error;
use wesichain_core::StoreError;

#[derive(Debug, Error)]
pub enum WeaviateStoreError {
    #[error("invalid configuration: base_url is required")]
    MissingBaseUrl,
    #[error("invalid configuration: base_url cannot be empty")]
    EmptyBaseUrl,
    #[error("invalid configuration: class_name is required")]
    MissingClassName,
    #[error("invalid configuration: class_name cannot be empty")]
    EmptyClassName,
    #[error("invalid configuration: class_name '{class_name}' {reason}")]
    InvalidClassName { class_name: String, reason: String },
    #[error("invalid document id: {0}")]
    InvalidDocumentId(String),
    #[error("document '{id}' is missing embedding")]
    MissingEmbedding { id: String },
    #[error("metadata filters are not supported yet")]
    UnsupportedFilter,
    #[error("unsupported metadata filter value for key '{key}': {reason}")]
    UnsupportedFilterValue { key: String, reason: String },
    #[error("request to weaviate failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("class '{class_name}' not found: {message}")]
    ClassNotFound { class_name: String, message: String },
    #[error("weaviate returned HTTP {status}: {message}")]
    HttpStatus { status: u16, message: String },
    #[error("invalid weaviate response: {message}")]
    InvalidResponse { message: String },
    #[error("weaviate object '{object_id}' is missing content payload '__wesichain_content'")]
    MissingContentPayload { object_id: String },
    #[error("weaviate object '{object_id}' has invalid metadata payload: {message}")]
    InvalidMetadataPayload { object_id: String, message: String },
}

impl From<WeaviateStoreError> for StoreError {
    fn from(value: WeaviateStoreError) -> Self {
        match value {
            WeaviateStoreError::InvalidDocumentId(id) => StoreError::InvalidId(id),
            other => StoreError::Internal(Box::new(other)),
        }
    }
}
