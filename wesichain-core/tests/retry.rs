use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

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

struct ParseFailer {
    attempts: Arc<AtomicUsize>,
}

impl ParseFailer {
    fn new() -> Self {
        Self {
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

struct TimeoutFlaky {
    failures_before_success: usize,
    attempts: Arc<AtomicUsize>,
}

impl TimeoutFlaky {
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

struct StreamTimeout {
    streams: Arc<AtomicUsize>,
}

impl StreamTimeout {
    fn new() -> Self {
        Self {
            streams: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn streams_counter(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.streams)
    }
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

#[async_trait::async_trait]
impl Runnable<String, String> for ParseFailer {
    async fn invoke(&self, _input: String) -> Result<String, WesichainError> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Err(WesichainError::ParseFailed {
            output: "bad".to_string(),
            reason: "invalid".to_string(),
        })
    }

    fn stream(
        &self,
        _input: String,
    ) -> BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        empty_stream()
    }
}

#[async_trait::async_trait]
impl Runnable<String, String> for TimeoutFlaky {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if attempt <= self.failures_before_success {
            return Err(WesichainError::Timeout(Duration::from_millis(50)));
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

#[async_trait::async_trait]
impl Runnable<String, String> for StreamTimeout {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(input)
    }

    fn stream(
        &self,
        _input: String,
    ) -> BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        self.streams.fetch_add(1, Ordering::SeqCst);
        let events = vec![Err(WesichainError::Timeout(Duration::from_secs(1)))];
        futures::stream::iter(events).boxed()
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

#[tokio::test]
async fn returns_max_retries_exceeded_immediately_when_max_attempts_zero() {
    let flaky = Flaky::new(1);
    let attempts = flaky.attempts_counter();
    let err = flaky
        .with_retries(0)
        .invoke("ping".to_string())
        .await
        .unwrap_err();

    assert!(matches!(err, WesichainError::MaxRetriesExceeded { max: 0 }));
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn non_retryable_error_fails_fast() {
    let parse_failer = ParseFailer::new();
    let attempts = parse_failer.attempts_counter();
    let err = parse_failer
        .with_retries(3)
        .invoke("ping".to_string())
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        WesichainError::ParseFailed {
            output: _,
            reason: _
        }
    ));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn stream_delegates_timeout_without_retrying() {
    let timeout_streamer = StreamTimeout::new();
    let streams = timeout_streamer.streams_counter();
    let retrying = timeout_streamer.with_retries(3);
    let mut stream = retrying.stream("ping".to_string());
    let first = stream.next().await.unwrap().unwrap_err();

    assert!(matches!(first, WesichainError::Timeout(_)));
    assert_eq!(streams.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn retries_timeout_errors_until_success() {
    let flaky = TimeoutFlaky::new(2);
    let attempts = flaky.attempts_counter();
    let output = flaky
        .with_retries(3)
        .invoke("ping".to_string())
        .await
        .unwrap();

    assert_eq!(output, "ok:ping".to_string());
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}
