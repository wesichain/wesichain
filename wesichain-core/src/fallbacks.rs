use std::sync::Arc;

use crate::{Runnable, StreamEvent, WesichainError};
use async_trait::async_trait;
use futures::stream::BoxStream;

pub struct RunnableWithFallbacks<Input, Output> {
    primary: Arc<dyn Runnable<Input, Output> + Send + Sync>,
    fallbacks: Vec<Arc<dyn Runnable<Input, Output> + Send + Sync>>,
}

impl<Input, Output> RunnableWithFallbacks<Input, Output> {
    pub fn new(
        primary: Arc<dyn Runnable<Input, Output> + Send + Sync>,
        fallbacks: Vec<Arc<dyn Runnable<Input, Output> + Send + Sync>>,
    ) -> Self {
        Self { primary, fallbacks }
    }
}

#[async_trait]
impl<Input, Output> Runnable<Input, Output> for RunnableWithFallbacks<Input, Output>
where
    Input: Clone + Send + Sync + 'static,
    Output: Send + Sync + 'static,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        let mut last_error = match self.primary.invoke(input.clone()).await {
            Ok(output) => return Ok(output),
            Err(e) => e,
        };

        for fallback in &self.fallbacks {
            match fallback.invoke(input.clone()).await {
                Ok(output) => return Ok(output),
                Err(e) => last_error = e,
            }
        }

        Err(last_error)
    }

    fn stream(&self, input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        // Streaming support for fallbacks is tricky (state reset).
        // Delegating to primary stream for now, but ignoring fallbacks in stream mode?
        // Or if primary fails stream, we can't easily switch to fallback stream mid-stream.
        // So we just stream primary.
        self.primary.stream(input)
    }

    fn to_serializable(&self) -> Option<crate::serde::SerializableRunnable> {
        let primary = Box::new(self.primary.to_serializable()?);
        let mut fallbacks = Vec::new();
        for f in &self.fallbacks {
            fallbacks.push(f.to_serializable()?);
        }
        Some(crate::serde::SerializableRunnable::Fallbacks { primary, fallbacks })
    }
}
