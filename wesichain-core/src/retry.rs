use futures::stream::BoxStream;

use crate::{Runnable, StreamEvent, WesichainError};

pub struct Retrying<R> {
    runnable: R,
    max_attempts: usize,
}

impl<R> Retrying<R> {
    pub fn new(runnable: R, max_attempts: usize) -> Self {
        Self {
            runnable,
            max_attempts,
        }
    }
}

pub fn is_retryable(error: &WesichainError) -> bool {
    matches!(
        error,
        WesichainError::LlmProvider(_)
            | WesichainError::ToolCallFailed { .. }
            | WesichainError::Timeout(_)
    )
}

#[async_trait::async_trait]
impl<Input, Output, R> Runnable<Input, Output> for Retrying<R>
where
    Input: Send + Clone + 'static,
    Output: Send + 'static,
    R: Runnable<Input, Output> + Send + Sync,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        if self.max_attempts == 0 {
            return Err(WesichainError::MaxRetriesExceeded { max: 0 });
        }

        for attempt in 1..=self.max_attempts {
            match self.runnable.invoke(input.clone()).await {
                Ok(output) => return Ok(output),
                Err(error) => {
                    if !is_retryable(&error) {
                        return Err(error);
                    }

                    if attempt == self.max_attempts {
                        return Err(WesichainError::MaxRetriesExceeded {
                            max: self.max_attempts,
                        });
                    }
                }
            }
        }

        Err(WesichainError::MaxRetriesExceeded {
            max: self.max_attempts,
        })
    }

    fn stream(&self, input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        self.runnable.stream(input)
    }
}
