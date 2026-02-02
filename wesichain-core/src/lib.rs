use async_trait::async_trait;
use futures::stream::BoxStream;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Token(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WesichainError {
    pub message: String,
}

impl WesichainError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[async_trait]
pub trait Runnable<Input, Output>: Send + Sync {
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError>;

    fn stream(&self, input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>>;
}

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
