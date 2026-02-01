use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WesichainError {
    #[error("Max retries ({max}) exceeded")]
    MaxRetriesExceeded { max: usize },
}
