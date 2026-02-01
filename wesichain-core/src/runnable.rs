use async_trait::async_trait;
use futures::stream::BoxStream;

use crate::WesichainError;

pub enum StreamEvent {
    ContentChunk(String),
    FinalAnswer(String),
}

#[async_trait]
pub trait Runnable<I, O> {
    async fn invoke(&self, input: I) -> Result<O, WesichainError>;

    fn stream<'a>(&'a self, input: I) -> BoxStream<'a, Result<StreamEvent, WesichainError>>;
}
