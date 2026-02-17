use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use wesichain_core::{
    CallbackHandler, CallbackManager, LlmInput, LlmResult, RunContext, RunType, Value,
};

#[derive(Clone)]
struct RecordingHandler {
    llm_starts: Arc<Mutex<Vec<(String, LlmInput)>>>,
    llm_ends: Arc<Mutex<Vec<(String, LlmResult)>>>,
    generic_starts: Arc<Mutex<Vec<String>>>,
    generic_ends: Arc<Mutex<Vec<String>>>,
}

impl RecordingHandler {
    fn new() -> Self {
        Self {
            llm_starts: Arc::new(Mutex::new(Vec::new())),
            llm_ends: Arc::new(Mutex::new(Vec::new())),
            generic_starts: Arc::new(Mutex::new(Vec::new())),
            generic_ends: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl CallbackHandler for RecordingHandler {
    async fn on_start(&self, ctx: &RunContext, _inputs: &Value) {
        self.generic_starts.lock().unwrap().push(ctx.name.clone());
    }

    async fn on_end(&self, ctx: &RunContext, _outputs: &Value, _duration_ms: u128) {
        self.generic_ends.lock().unwrap().push(ctx.name.clone());
    }

    async fn on_error(&self, _ctx: &RunContext, _error: &Value, _duration_ms: u128) {}

    async fn on_llm_start(&self, ctx: &RunContext, input: &LlmInput) {
        self.llm_starts
            .lock()
            .unwrap()
            .push((ctx.name.clone(), input.clone()));
    }

    async fn on_llm_end(&self, ctx: &RunContext, result: &LlmResult, _duration_ms: u128) {
        self.llm_ends
            .lock()
            .unwrap()
            .push((ctx.name.clone(), result.clone()));
    }
}

#[tokio::test]
async fn callback_handler_llm_methods_are_called() {
    let handler = Arc::new(RecordingHandler::new());
    let manager = CallbackManager::new(vec![handler.clone()]);

    let ctx = RunContext::root(
        RunType::Llm,
        "test-llm".to_string(),
        vec![],
        BTreeMap::new(),
    );
    let input = LlmInput {
        model: "gpt-4".to_string(),
        prompt: "Hello".to_string(),
        temperature: Some(0.5),
        max_tokens: Some(100),
        stop_sequences: vec![],
    };

    manager.on_llm_start(&ctx, &input).await;

    let starts = handler.llm_starts.lock().unwrap();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].0, "test-llm");
    assert_eq!(starts[0].1.model, "gpt-4");
}

#[tokio::test]
async fn default_impl_fallback_to_on_start() {
    struct FallbackHandler {
        starts: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl CallbackHandler for FallbackHandler {
        async fn on_start(&self, ctx: &RunContext, _inputs: &Value) {
            self.starts.lock().unwrap().push(ctx.name.clone());
        }
        async fn on_end(&self, _ctx: &RunContext, _outputs: &Value, _duration_ms: u128) {}
        async fn on_error(&self, _ctx: &RunContext, _error: &Value, _duration_ms: u128) {}
    }

    let handler = Arc::new(FallbackHandler {
        starts: Arc::new(Mutex::new(Vec::new())),
    });
    let manager = CallbackManager::new(vec![handler.clone()]);

    let ctx = RunContext::root(
        RunType::Llm,
        "fallback-test".to_string(),
        vec![],
        BTreeMap::new(),
    );
    let input = LlmInput {
        model: "gpt-3".to_string(),
        prompt: "Test".to_string(),
        temperature: None,
        max_tokens: None,
        stop_sequences: vec![],
    };

    // Calls on_llm_start which has default impl calling on_start
    manager.on_llm_start(&ctx, &input).await;

    let starts = handler.starts.lock().unwrap();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0], "fallback-test");
}
