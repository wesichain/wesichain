use thiserror::Error;

#[derive(Debug, Error)]
#[error("retrieval error")]
pub struct RetrievalError;
