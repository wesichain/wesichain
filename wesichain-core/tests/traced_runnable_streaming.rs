use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use futures::stream::{self, BoxStream, StreamExt};
use wesichain_core::{
    CallbackHandler, CallbackManager, RunContext, RunType, Runnable, StreamEvent, TracedRunnable,
    Value, WesichainError,
};

#[derive(Clone)]
struct RecordingHandler {
    events: Arc<Mutex<Vec<String>>>,
}

impl RecordingHandler {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl CallbackHandler for RecordingHandler {
    async fn on_start(&self, _ctx: &RunContext, _inputs: &Value) {
        self.events.lock().unwrap().push("start".to_string());
    }

    async fn on_end(&self, _ctx: &RunContext, _outputs: &Value, _duration_ms: u128) {
        self.events.lock().unwrap().push("end".to_string());
    }

    async fn on_error(&self, _ctx: &RunContext, _error: &Value, _duration_ms: u128) {
        self.events.lock().unwrap().push("error".to_string());
    }

    async fn on_stream_chunk(&self, _ctx: &RunContext, _chunk: &Value) {
        self.events.lock().unwrap().push("chunk".to_string());
    }
}

struct MockStreamingRunnable;

#[async_trait::async_trait]
impl Runnable<String, String> for MockStreamingRunnable {
    async fn invoke(&self, _input: String) -> Result<String, WesichainError> {
        Ok("result".to_string())
    }

    fn stream(
        &self,
        _input: String,
    ) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::iter(vec![
            Ok(StreamEvent::ContentChunk("Hello".to_string())),
            Ok(StreamEvent::ContentChunk(" World".to_string())),
        ])
        .boxed()
    }
}

#[tokio::test]
async fn traced_runnable_stream_fires_callbacks() {
    let handler = Arc::new(RecordingHandler::new());
    let manager = CallbackManager::new(vec![handler.clone()]);

    let parent = RunContext::root(RunType::Chain, "parent".to_string(), vec![], BTreeMap::new());

    let inner = MockStreamingRunnable;
    let traced = TracedRunnable::new(
        inner,
        manager,
        parent,
        RunType::Chain,
        "test".to_string(),
    );

    let mut stream = traced.stream("input".to_string());
    while let Some(_event) = stream.next().await {}

    let events = handler.events.lock().unwrap();
    assert!(
        events.contains(&"start".to_string()),
        "should have fired on_start"
    );
    assert!(
        events.contains(&"chunk".to_string()),
        "should have fired on_stream_chunk"
    );
    assert!(
        events.contains(&"end".to_string()),
        "should have fired on_end"
    );
}
