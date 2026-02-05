use std::sync::{Arc, Mutex};

use futures::StreamExt;
use wesichain_core::callbacks::{
    CallbackHandler, CallbackManager, RunContext, RunType, TracedRunnable,
};
use wesichain_core::{Runnable, Value, WesichainError};

struct RecordingHandler {
    events: Arc<Mutex<Vec<String>>>,
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
}

struct OkRunnable;

#[async_trait::async_trait]
impl Runnable<String, String> for OkRunnable {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(format!("{}!", input))
    }

    fn stream(
        &self,
        _input: String,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
async fn traced_runnable_emits_start_and_end() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let handler = Arc::new(RecordingHandler {
        events: events.clone(),
    });
    let manager = CallbackManager::new(vec![handler]);
    let root = RunContext::root(
        RunType::Chain,
        "root".to_string(),
        vec![],
        Default::default(),
    );

    let traced = TracedRunnable::new(
        OkRunnable,
        manager,
        root,
        RunType::Chain,
        "node".to_string(),
    );
    let _ = traced.invoke("hi".to_string()).await.unwrap();

    let events = events.lock().unwrap().clone();
    assert_eq!(events, vec!["start", "end"]);
}
