use async_trait::async_trait;
use futures::stream::BoxStream;

use crate::WesichainError;

#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    ContentChunk(String),
    ToolCallStart { id: String, name: String },
    ToolCallDelta { id: String, delta: crate::Value },
    ToolCallResult { id: String, output: crate::Value },
    FinalAnswer(String),
    Metadata { key: String, value: crate::Value },
}

#[async_trait]
pub trait Runnable<Input: Send + 'static, Output: Send + 'static> {
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError>;

    fn stream(&self, input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>>;
}
