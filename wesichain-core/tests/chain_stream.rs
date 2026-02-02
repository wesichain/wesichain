use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use futures::stream::{BoxStream, StreamExt};

use wesichain_core::{Runnable, RunnableExt, StreamEvent, WesichainError};

struct HeadCounter {
    calls: Arc<AtomicUsize>,
}

impl HeadCounter {
    fn new() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn calls_counter(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.calls)
    }
}

struct HeadParseFail;

struct TailStreamer;

fn empty_stream<'a>() -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
    futures::stream::iter(vec![Ok(StreamEvent::FinalAnswer(String::new()))]).boxed()
}

#[async_trait::async_trait]
impl Runnable<String, String> for HeadCounter {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(format!("mid:{input}"))
    }

    fn stream(&self, _input: String) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        empty_stream()
    }
}

#[async_trait::async_trait]
impl Runnable<String, String> for HeadParseFail {
    async fn invoke(&self, _input: String) -> Result<String, WesichainError> {
        Err(WesichainError::ParseFailed {
            output: "bad".to_string(),
            reason: "invalid".to_string(),
        })
    }

    fn stream(&self, _input: String) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        empty_stream()
    }
}

#[async_trait::async_trait]
impl Runnable<String, String> for TailStreamer {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(format!("tail:{input}"))
    }

    fn stream(&self, _input: String) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        let events = vec![
            Ok(StreamEvent::ContentChunk("chunk".to_string())),
            Ok(StreamEvent::FinalAnswer("done".to_string())),
        ];
        futures::stream::iter(events).boxed()
    }
}

#[tokio::test]
async fn chain_streams_tail_events_after_head_invoke() {
    let head = HeadCounter::new();
    let calls = head.calls_counter();
    let chain = head.then(TailStreamer);
    let events: Vec<_> = chain.stream("ping".to_string()).collect().await;
    let events: Vec<_> = events.into_iter().collect::<Result<Vec<_>, _>>().unwrap();

    let expected = vec![
        StreamEvent::ContentChunk("chunk".to_string()),
        StreamEvent::FinalAnswer("done".to_string()),
    ];

    assert_eq!(events, expected);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn chain_stream_propagates_head_error() {
    let chain = HeadParseFail.then(TailStreamer);
    let mut stream = chain.stream("ping".to_string());
    let first = stream.next().await.unwrap().unwrap_err();

    assert!(matches!(
        first,
        WesichainError::ParseFailed {
            output: _,
            reason: _
        }
    ));
}
