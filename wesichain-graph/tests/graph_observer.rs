use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{GraphBuilder, GraphState, Observer, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

struct Inc;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for Inc {
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
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[derive(Default)]
struct CollectingObserver {
    events: Arc<Mutex<Vec<String>>>,
}

impl Observer for CollectingObserver {
    fn on_node_enter(&self, node: &str) {
        self.events
            .lock()
            .unwrap()
            .push(format!("enter:{node}"));
    }

    fn on_node_exit(&self, node: &str) {
        self.events
            .lock()
            .unwrap()
            .push(format!("exit:{node}"));
    }
}

#[tokio::test]
async fn observer_receives_node_events() {
    let observer = CollectingObserver::default();
    let events = observer.events.clone();
    let graph = GraphBuilder::new()
        .add_node("inc", Inc)
        .set_entry("inc")
        .with_observer(Arc::new(observer))
        .build();

    let state = GraphState::new(DemoState { count: 0 });
    graph.invoke_graph(state).await.unwrap();
    assert_eq!(events.lock().unwrap().as_slice(), ["enter:inc", "exit:inc"]);
}
