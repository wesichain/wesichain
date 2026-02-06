use std::path::PathBuf;

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

#[derive(Debug, Error)]
pub enum IngestionError {
    #[error("dispatch failed for '{path}': file extension is missing")]
    MissingExtension { path: PathBuf },
    #[error("dispatch failed for '{path}': unsupported extension '{extension}'")]
    UnsupportedExtension { path: PathBuf, extension: String },
    #[error("read failed for '{path}': {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
