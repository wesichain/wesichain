use async_trait::async_trait;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use wesichain_core::{Runnable, WesichainError};
use wesichain_graph::{GraphBuilder, GraphError, GraphState, Observer, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
struct TestState {
    data: String,
}

impl StateSchema for TestState {
    type Update = TestState;
    fn apply(current: &Self, update: TestState) -> Self {
        TestState {
            data: format!("{}{}", current.data, update.data),
        }
    }
}

// Mock Observer
struct MockObserver {
    events: Arc<Mutex<Vec<String>>>,
}

impl MockObserver {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Observer for MockObserver {
    async fn on_node_start(&self, node: &str, _input: &Value) {
        self.events.lock().unwrap().push(format!("start:{}", node));
    }

    async fn on_node_end(&self, node: &str, _output: &Value, _duration: u128) {
        self.events.lock().unwrap().push(format!("end:{}", node));
    }

    async fn on_error(&self, node: &str, _error: &GraphError) {
        self.events.lock().unwrap().push(format!("error:{}", node));
    }

    async fn on_checkpoint_saved(&self, thread_id: &str) {
        self.events
            .lock()
            .unwrap()
            .push(format!("checkpoint:{}", thread_id));
    }
}

#[derive(Clone)]
struct TestNode;

#[async_trait]
impl Runnable<GraphState<TestState>, StateUpdate<TestState>> for TestNode {
    async fn invoke(
        &self,
        _input: GraphState<TestState>,
    ) -> Result<StateUpdate<TestState>, WesichainError> {
        Ok(StateUpdate::new(TestState {
            data: "_n1".to_string(),
        }))
    }

    fn stream<'a>(
        &'a self,
        _input: GraphState<TestState>,
    ) -> std::pin::Pin<
        Box<
            dyn futures::Stream<Item = Result<wesichain_core::StreamEvent, WesichainError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(futures::stream::empty())
    }
}

#[tokio::test]
async fn test_unified_observability() {
    let observer = Arc::new(MockObserver::new());

    let node1 = TestNode;

    let builder = GraphBuilder::<TestState>::new()
        .add_node("node1", node1)
        .set_entry("node1")
        .with_observer(observer.clone());

    let graph = builder.build();

    let initial = GraphState::new(TestState {
        data: "init".to_string(),
    });

    // Run using Runnable trait
    let _ = Runnable::invoke(&graph, initial).await.unwrap();

    // Verify events
    let events = observer.events.lock().unwrap().clone();
    println!("Events: {:?}", events);

    assert!(events.contains(&"start:node1".to_string()));
    assert!(events.contains(&"end:node1".to_string()));
}

#[tokio::test]
async fn test_observer_backward_compat() {
    // Verify that a legacy Observer wired via `.with_observer()` still receives
    // on_node_start, on_node_end, and on_checkpoint_saved events through the
    // ObserverCallbackAdapter bridge.

    let observer = Arc::new(MockObserver::new());

    #[derive(Clone)]
    struct NodeA;

    #[async_trait]
    impl Runnable<GraphState<TestState>, StateUpdate<TestState>> for NodeA {
        async fn invoke(
            &self,
            _input: GraphState<TestState>,
        ) -> Result<StateUpdate<TestState>, WesichainError> {
            Ok(StateUpdate::new(TestState {
                data: "_a".to_string(),
            }))
        }
        fn stream<'a>(
            &'a self,
            _input: GraphState<TestState>,
        ) -> std::pin::Pin<
            Box<
                dyn futures::Stream<Item = Result<wesichain_core::StreamEvent, WesichainError>>
                    + Send
                    + 'a,
            >,
        > {
            Box::pin(futures::stream::empty())
        }
    }

    #[derive(Clone)]
    struct NodeB;

    #[async_trait]
    impl Runnable<GraphState<TestState>, StateUpdate<TestState>> for NodeB {
        async fn invoke(
            &self,
            _input: GraphState<TestState>,
        ) -> Result<StateUpdate<TestState>, WesichainError> {
            Ok(StateUpdate::new(TestState {
                data: "_b".to_string(),
            }))
        }
        fn stream<'a>(
            &'a self,
            _input: GraphState<TestState>,
        ) -> std::pin::Pin<
            Box<
                dyn futures::Stream<Item = Result<wesichain_core::StreamEvent, WesichainError>>
                    + Send
                    + 'a,
            >,
        > {
            Box::pin(futures::stream::empty())
        }
    }

    // Build a 2-node graph with observer and checkpointer
    let checkpointer = wesichain_core::checkpoint::InMemoryCheckpointer::<TestState>::default();

    let builder = GraphBuilder::<TestState>::new()
        .add_node("node_a", NodeA)
        .add_node("node_b", NodeB)
        .add_edge("node_a", "node_b")
        .set_entry("node_a")
        .with_observer(observer.clone())
        .with_checkpointer(checkpointer, "test_thread");

    let graph = builder.build();
    let initial = GraphState::new(TestState {
        data: "init".to_string(),
    });

    let result = Runnable::invoke(&graph, initial).await.unwrap();
    assert_eq!(result.data.data, "init_a_b");

    // Verify observer received all expected events in order
    let events = observer.events.lock().unwrap().clone();
    println!("Backward-compat events: {:?}", events);

    // Node events
    assert!(
        events.contains(&"start:node_a".to_string()),
        "Missing start:node_a"
    );
    assert!(
        events.contains(&"end:node_a".to_string()),
        "Missing end:node_a"
    );
    assert!(
        events.contains(&"start:node_b".to_string()),
        "Missing start:node_b"
    );
    assert!(
        events.contains(&"end:node_b".to_string()),
        "Missing end:node_b"
    );

    // Checkpoint events (one per node completion)
    let checkpoint_events: Vec<_> = events
        .iter()
        .filter(|e| e.starts_with("checkpoint:"))
        .collect();
    assert!(
        !checkpoint_events.is_empty(),
        "Observer should receive on_checkpoint_saved events, got: {:?}",
        events
    );

    // Verify ordering: start:node_a must come before end:node_a
    let start_a_pos = events.iter().position(|e| e == "start:node_a").unwrap();
    let end_a_pos = events.iter().position(|e| e == "end:node_a").unwrap();
    let start_b_pos = events.iter().position(|e| e == "start:node_b").unwrap();
    let end_b_pos = events.iter().position(|e| e == "end:node_b").unwrap();
    assert!(
        start_a_pos < end_a_pos,
        "start:node_a should come before end:node_a"
    );
    assert!(
        end_a_pos < start_b_pos,
        "end:node_a should come before start:node_b"
    );
    assert!(
        start_b_pos < end_b_pos,
        "start:node_b should come before end:node_b"
    );
}
