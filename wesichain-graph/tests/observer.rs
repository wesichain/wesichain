use std::sync::{Arc, Mutex};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, WesichainError};
use wesichain_graph::{
    ExecutionOptions, GraphBuilder, GraphState, Observer, StateSchema, StateUpdate,
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {
    type Update = Self;
    fn apply(_: &Self, update: Self) -> Self {
        update
    }
}

struct AddOne;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for AddOne {
    async fn invoke(
        &self,
        input: GraphState<DemoState>,
    ) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState {
            count: input.data.count + 1,
        }))
    }

    fn stream(
        &self,
        _input: GraphState<DemoState>,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[derive(Default)]
struct CollectingObserver {
    events: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl Observer for CollectingObserver {
    async fn on_node_start(&self, node_id: &str, _input: &serde_json::Value) {
        self.events.lock().unwrap().push(format!("start:{node_id}"));
    }

    async fn on_node_end(&self, node_id: &str, _output: &serde_json::Value, _duration_ms: u128) {
        self.events.lock().unwrap().push(format!("end:{node_id}"));
    }

    async fn on_error(&self, node_id: &str, _error: &wesichain_graph::GraphError) {
        self.events.lock().unwrap().push(format!("error:{node_id}"));
    }
}

#[tokio::test]
async fn observer_receives_node_events() {
    let observer = CollectingObserver::default();
    let events = observer.events.clone();
    let graph = GraphBuilder::new()
        .add_node("add", AddOne)
        .set_entry("add")
        .build();
    let options = ExecutionOptions {
        observer: Some(Arc::new(observer)),
        ..ExecutionOptions::default()
    };

    let state = GraphState::new(DemoState { count: 0 });
    let _ = graph.invoke_with_options(state, options).await.unwrap();
    let captured = events.lock().unwrap().clone();
    assert_eq!(captured, vec!["start:add", "end:add"]);
}
