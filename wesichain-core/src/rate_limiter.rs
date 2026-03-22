use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::stream::BoxStream;
use futures::StreamExt as _;
use tokio::sync::Mutex;

use crate::{Runnable, StreamEvent, WesichainError};

pub struct RateLimited<R> {
    inner: R,
    interval: Duration,
    last_call: Arc<Mutex<Option<Instant>>>,
}

impl<R> RateLimited<R> {
    pub fn new(inner: R, requests_per_minute: u32) -> Self {
        let interval = if requests_per_minute == 0 {
            Duration::from_secs(u64::MAX / 2)
        } else {
            Duration::from_secs(60) / requests_per_minute
        };
        Self {
            inner,
            interval,
            last_call: Arc::new(Mutex::new(None)),
        }
    }

    async fn throttle(&self) {
        let mut last = self.last_call.lock().await;
        if let Some(t) = *last {
            let elapsed = t.elapsed();
            if elapsed < self.interval {
                tokio::time::sleep(self.interval - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }
}

#[async_trait::async_trait]
impl<Input, Output, R> Runnable<Input, Output> for RateLimited<R>
where
    Input: Send + Clone + 'static,
    Output: Send + 'static,
    R: Runnable<Input, Output> + Send + Sync,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        self.throttle().await;
        self.inner.invoke(input).await
    }

    fn stream<'a>(&'a self, input: Input) -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
        let inner = &self.inner;
        async_stream::stream! {
            self.throttle().await;
            let mut s = inner.stream(input);
            while let Some(event) = s.next().await {
                yield event;
            }
        }
        .boxed()
    }
}
