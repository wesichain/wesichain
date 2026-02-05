use std::sync::{Arc, Mutex};

use futures::StreamExt;
use wesichain_core::callbacks::{CallbackHandler, CallbackManager, RunConfig, RunContext};
use wesichain_core::{Runnable, Value, WesichainError};
use wesichain_graph::{ExecutionOptions, GraphBuilder, GraphState, StateSchema, StateUpdate};

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct DemoState {
    value: usize,
}

impl StateSchema for DemoState {}

struct IncrNode;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for IncrNode {
    async fn invoke(
        &self,
        input: GraphState<DemoState>,
    ) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState {
            value: input.data.value + 1,
        }))
    }

    fn stream(
        &self,
        _input: GraphState<DemoState>,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

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

    async fn on_error(&self, _ctx: &RunContext, _error: &Value, _duration_ms: u128) {}
}

#[tokio::test]
async fn graph_invocation_emits_callbacks() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let handler = Arc::new(RecordingHandler {
        events: events.clone(),
    });
    let callbacks = CallbackManager::new(vec![handler]);

    let options = ExecutionOptions {
        run_config: Some(RunConfig {
            callbacks: Some(callbacks),
            ..Default::default()
        }),
        ..Default::default()
    };

    let graph = GraphBuilder::new()
        .add_node("node", IncrNode)
        .set_entry("node")
        .build();

    let _ = graph
        .invoke_with_options(GraphState::new(DemoState::default()), options)
        .await
        .unwrap();

    let events = events.lock().unwrap().clone();
    assert!(events.len() >= 2);
}
