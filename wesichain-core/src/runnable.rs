use async_trait::async_trait;
use futures::stream::BoxStream;
use std::sync::Arc;

use crate::{serde::SerializableRunnable, WesichainError};

#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    ContentChunk(String),
    ToolCallStart { id: String, name: String },
    ToolCallDelta { id: String, delta: crate::Value },
    ToolCallResult { id: String, output: crate::Value },
    FinalAnswer(String),
    Metadata { key: String, value: crate::Value },
    /// Emitted when an agent pauses at a human-in-the-loop approval gate.
    ///
    /// The host application should present `prompt` to a human, collect their
    /// decision, and resume the graph from `checkpoint_id`.
    AwaitingApproval {
        /// Stable identifier for this execution run.
        run_id: String,
        /// Human-readable description of what the agent wants to do.
        prompt: String,
        /// Opaque checkpoint identifier — pass back to resume the graph.
        checkpoint_id: String,
    },
    /// Model reasoning / scratchpad text (Anthropic extended thinking, OpenAI o-series).
    ThinkingChunk(String),
    /// Token consumption update emitted after each LLM response.
    UsageUpdate {
        input_tokens: u32,
        output_tokens: u32,
        cache_read_tokens: Option<u32>,
        cache_write_tokens: Option<u32>,
    },
}

#[async_trait]
pub trait Runnable<Input: Send + 'static, Output: Send + 'static>: Send + Sync {
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError>;

    async fn batch(&self, inputs: Vec<Input>) -> Vec<Result<Output, WesichainError>> {
        let futures = inputs.into_iter().map(|i| self.invoke(i));
        futures::future::join_all(futures).await
    }

    async fn abatch(&self, inputs: Vec<Input>) -> Vec<Result<Output, WesichainError>> {
        self.batch(inputs).await
    }

    fn to_serializable(&self) -> Option<SerializableRunnable> {
        None
    }

    fn stream<'a>(&'a self, input: Input) -> BoxStream<'a, Result<StreamEvent, WesichainError>>;
}

#[async_trait]
impl<Input, Output, T> Runnable<Input, Output> for Arc<T>
where
    Input: Send + 'static,
    Output: Send + 'static,
    T: Runnable<Input, Output> + ?Sized,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        (**self).invoke(input).await
    }

    fn stream<'a>(&'a self, input: Input) -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
        (**self).stream(input)
    }

    fn to_serializable(&self) -> Option<SerializableRunnable> {
        (**self).to_serializable()
    }
}

#[async_trait]
impl<Input, Output, T> Runnable<Input, Output> for Box<T>
where
    Input: Send + 'static,
    Output: Send + 'static,
    T: Runnable<Input, Output> + ?Sized,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        (**self).invoke(input).await
    }

    fn stream<'a>(&'a self, input: Input) -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
        (**self).stream(input)
    }

    fn to_serializable(&self) -> Option<SerializableRunnable> {
        (**self).to_serializable()
    }
}
