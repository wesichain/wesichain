use std::time::Duration;

use futures::stream::BoxStream;
use futures::StreamExt as _;

use crate::{Runnable, StreamEvent, WesichainError};

pub struct TimeLimited<R> {
    inner: R,
    timeout: Duration,
}

impl<R> TimeLimited<R> {
    pub fn new(inner: R, timeout: Duration) -> Self {
        Self { inner, timeout }
    }
}

#[async_trait::async_trait]
impl<Input, Output, R> Runnable<Input, Output> for TimeLimited<R>
where
    Input: Send + Clone + 'static,
    Output: Send + 'static,
    R: Runnable<Input, Output> + Send + Sync,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        tokio::time::timeout(self.timeout, self.inner.invoke(input))
            .await
            .map_err(|_| WesichainError::Timeout(self.timeout))?
    }

    fn stream<'a>(&'a self, input: Input) -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
        let timeout = self.timeout;
        let inner = &self.inner;
        async_stream::stream! {
            let mut s = inner.stream(input);
            loop {
                match tokio::time::timeout(timeout, s.next()).await {
                    Err(_) => {
                        yield Err(WesichainError::Timeout(timeout));
                        break;
                    }
                    Ok(None) => break,
                    Ok(Some(event)) => yield event,
                }
            }
        }
        .boxed()
    }
}
