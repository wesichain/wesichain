use futures::stream::BoxStream;
use rand::Rng;

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
            | WesichainError::RateLimitExceeded { .. }
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

        let mut attempt = 0;
        loop {
            attempt += 1;
            match self.runnable.invoke(input.clone()).await {
                Ok(output) => return Ok(output),
                Err(error) => {
                    if !is_retryable(&error) || attempt >= self.max_attempts {
                        if attempt >= self.max_attempts {
                            return Err(WesichainError::MaxRetriesExceeded {
                                max: self.max_attempts,
                            });
                        }
                        return Err(error);
                    }

                    // Exponential backoff: base 100ms * 2^(attempt-1)
                    // Cap at ~10s (attempt 7+) to avoid excessive delays in interactive apps
                    let base_delay_ms = 100u64 * (1u64 << (attempt - 1).min(7));
                    let jitter_ms = rand::thread_rng().gen_range(0..100);
                    let delay = std::time::Duration::from_millis(base_delay_ms + jitter_ms);

                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// Retry-on-stream-start: if the stream errors before its first item is emitted,
    /// apply exponential backoff and re-attempt (up to `max_attempts`).
    /// Once streaming is in progress (first item emitted), errors pass through as-is.
    fn stream<'a>(&'a self, input: Input) -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
        use futures::StreamExt as _;
        let runnable = &self.runnable;
        let max_attempts = self.max_attempts;

        async_stream::stream! {
            if max_attempts == 0 {
                yield Err(WesichainError::MaxRetriesExceeded { max: 0 });
                return;
            }

            let mut attempt = 0usize;
            loop {
                attempt += 1;
                let mut inner = runnable.stream(input.clone());

                match inner.next().await {
                    None => break,
                    Some(first) => {
                        if matches!(&first, Err(e) if is_retryable(e) && attempt < max_attempts) {
                            let base_delay_ms = 100u64 * (1u64 << (attempt - 1).min(7));
                            let jitter_ms = rand::thread_rng().gen_range(0..100u64);
                            let delay = std::time::Duration::from_millis(base_delay_ms + jitter_ms);
                            tokio::time::sleep(delay).await;
                            continue;
                        }

                        // Exhausted retries on a retryable error → emit MaxRetriesExceeded
                        let item = match first {
                            Err(ref e) if is_retryable(e) => {
                                Err(WesichainError::MaxRetriesExceeded { max: max_attempts })
                            }
                            item => item,
                        };
                        yield item;
                        while let Some(event) = inner.next().await {
                            yield event;
                        }
                        break;
                    }
                }
            }
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use futures::StreamExt as _;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct FailRunnable {
        failures: usize,
        count: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl Runnable<(), ()> for FailRunnable {
        async fn invoke(&self, _: ()) -> Result<(), WesichainError> {
            let current = self.count.fetch_add(1, Ordering::SeqCst);
            if current < self.failures {
                Err(WesichainError::Timeout(std::time::Duration::from_millis(1)))
            } else {
                Ok(())
            }
        }

        fn stream<'a>(&'a self, _: ()) -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
            let current = self.count.fetch_add(1, Ordering::SeqCst);
            if current < self.failures {
                stream::iter(vec![Err(WesichainError::Timeout(
                    std::time::Duration::from_millis(1),
                ))])
                .boxed()
            } else {
                stream::iter(vec![Ok(StreamEvent::ContentChunk("ok".to_string()))]).boxed()
            }
        }
    }

    #[tokio::test]
    async fn test_retry_success() {
        let count = Arc::new(AtomicUsize::new(0));
        let runnable = FailRunnable {
            failures: 2,
            count: count.clone(),
        };
        let retrying = Retrying::new(runnable, 3);

        let start = std::time::Instant::now();
        retrying.invoke(()).await.unwrap();
        let elapsed = start.elapsed();

        assert_eq!(count.load(Ordering::SeqCst), 3); // 2 fails + 1 success
                                                     // Base delays: 100ms (attempt 1) + 200ms (attempt 2) = 300ms minimum
        assert!(elapsed.as_millis() >= 300);
    }

    #[tokio::test]
    async fn test_max_retries_exceeded() {
        let count = Arc::new(AtomicUsize::new(0));
        let runnable = FailRunnable {
            failures: 5,
            count: count.clone(),
        };
        let retrying = Retrying::new(runnable, 3);

        let result = retrying.invoke(()).await;
        assert!(matches!(
            result,
            Err(WesichainError::MaxRetriesExceeded { max: 3 })
        ));
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_stream_retry_on_first_item_error() {
        // Stream fails on first 2 attempts, succeeds on 3rd
        let count = Arc::new(AtomicUsize::new(0));
        let runnable = FailRunnable {
            failures: 2,
            count: count.clone(),
        };
        let retrying = Retrying::new(runnable, 3);

        let events: Vec<_> = retrying.stream(()).collect().await;
        // Should succeed on 3rd attempt with one ContentChunk
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], Ok(StreamEvent::ContentChunk(_))));
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_stream_max_retries_exceeded_yields_error() {
        // Stream always fails
        let count = Arc::new(AtomicUsize::new(0));
        let runnable = FailRunnable {
            failures: 10,
            count: count.clone(),
        };
        let retrying = Retrying::new(runnable, 3);

        let events: Vec<_> = retrying.stream(()).collect().await;
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            Err(WesichainError::MaxRetriesExceeded { max: 3 })
        ));
    }

    #[tokio::test]
    async fn test_stream_zero_max_attempts_yields_error() {
        let count = Arc::new(AtomicUsize::new(0));
        let runnable = FailRunnable {
            failures: 0,
            count: count.clone(),
        };
        let retrying = Retrying::new(runnable, 0);

        let events: Vec<_> = retrying.stream(()).collect().await;
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            Err(WesichainError::MaxRetriesExceeded { max: 0 })
        ));
    }
}
