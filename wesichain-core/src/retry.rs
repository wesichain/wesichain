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

    /// Retries apply to invoke only; stream delegates in v0.
    fn stream(&self, input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        self.runnable.stream(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        fn stream(&self, _: ()) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
            unimplemented!()
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
}
