use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use futures::stream::{BoxStream, StreamExt};

use wesichain_core::{Runnable, RunnableExt, WesichainError};

struct Flaky {
    failures_before_success: usize,
    attempts: Arc<AtomicUsize>,
}

impl Flaky {
    fn new(failures_before_success: usize) -> Self {
        Self {
            failures_before_success,
            attempts: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn attempts_counter(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.attempts)
    }
}

fn empty_stream<'a>() -> BoxStream<'a, Result<wesichain_core::StreamEvent, WesichainError>> {
    futures::stream::iter(vec![Ok(wesichain_core::StreamEvent::FinalAnswer(
        String::new(),
    ))])
    .boxed()
}

#[async_trait::async_trait]
impl Runnable<String, String> for Flaky {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if attempt <= self.failures_before_success {
            return Err(WesichainError::LlmProvider("transient".to_string()));
        }

        Ok(format!("ok:{input}"))
    }

    fn stream(
        &self,
        _input: String,
    ) -> BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        empty_stream()
    }
}

#[tokio::test]
async fn retries_until_success() {
    let flaky = Flaky::new(2);
    let attempts = flaky.attempts_counter();
    let output = flaky
        .with_retries(3)
        .invoke("ping".to_string())
        .await
        .unwrap();

    assert_eq!(output, "ok:ping".to_string());
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn returns_max_retries_exceeded() {
    let flaky = Flaky::new(5);
    let attempts = flaky.attempts_counter();
    let err = flaky
        .with_retries(2)
        .invoke("ping".to_string())
        .await
        .unwrap_err();

    assert!(matches!(err, WesichainError::MaxRetriesExceeded { max: 2 }));
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}
