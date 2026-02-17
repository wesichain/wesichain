use futures::stream::{self, BoxStream, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{
    Checkpointer, ExecutionOptions, GraphBuilder, GraphError, GraphState, InMemoryCheckpointer,
    StateSchema, StateUpdate,
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    executed: Vec<String>,
}

impl StateSchema for DemoState {
    fn merge(current: &Self, other: Self) -> Self {
        let mut executed = current.executed.clone();
        executed.extend(other.executed);
        Self { executed }
    }
}

struct RecordNode {
    name: String,
    delay: Option<Duration>,
}

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for RecordNode {
    async fn invoke(
        &self,
        _: GraphState<DemoState>,
    ) -> Result<StateUpdate<DemoState>, WesichainError> {
        if let Some(d) = self.delay {
            sleep(d).await;
        }
        Ok(StateUpdate::new(DemoState {
            executed: vec![self.name.clone()],
        }))
    }

    fn stream<'a>(
        &'a self,
        _input: GraphState<DemoState>,
    ) -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
        stream::empty().boxed()
    }
}

#[tokio::test]
async fn test_interrupt_after_resume() {
    let checkpointer = InMemoryCheckpointer::default();
    let graph = GraphBuilder::<DemoState>::new()
        .add_node(
            "A",
            RecordNode {
                name: "A".to_string(),
                delay: None,
            },
        )
        .add_node(
            "B",
            RecordNode {
                name: "B".to_string(),
                delay: None,
            },
        )
        .add_node(
            "C",
            RecordNode {
                name: "C".to_string(),
                delay: None,
            },
        )
        .add_edge("A", "B")
        .add_edge("B", "C")
        .set_entry("A")
        .with_checkpointer(checkpointer.clone(), "thread-1")
        .build();

    let options = ExecutionOptions {
        interrupt_after: vec!["A".to_string()],
        ..Default::default()
    };

    let result = graph
        .invoke_graph_with_options(GraphState::new(DemoState::default()), options)
        .await;

    // Should fail with Interrupted
    match result {
        Err(GraphError::Interrupted) => {}
        _ => panic!("Expected Interrupted error, got {:?}", result),
    }

    // Load Checkpoint
    let checkpoint = checkpointer
        .load("thread-1")
        .await
        .unwrap()
        .expect("Checkpoint not found");
    assert_eq!(checkpoint.node, "A");
    // Queue should have "B" (successor of A)
    assert!(checkpoint.queue.iter().any(|(n, _)| n == "B"));

    // Resume
    let resumed = graph
        .resume(checkpoint, ExecutionOptions::default())
        .await
        .unwrap();

    assert_eq!(resumed.data.executed, vec!["A", "B", "C"]);
}

#[tokio::test]
async fn test_interrupt_before_resume() {
    let checkpointer = InMemoryCheckpointer::default();
    let graph = GraphBuilder::<DemoState>::new()
        .add_node(
            "A",
            RecordNode {
                name: "A".to_string(),
                delay: None,
            },
        )
        .add_node(
            "B",
            RecordNode {
                name: "B".to_string(),
                delay: None,
            },
        )
        .add_edge("A", "B")
        .set_entry("A")
        .with_checkpointer(checkpointer.clone(), "thread-2")
        .build();

    let options = ExecutionOptions {
        interrupt_before: vec!["B".to_string()],
        ..Default::default()
    };

    let result = graph
        .invoke_graph_with_options(GraphState::new(DemoState::default()), options)
        .await;

    match result {
        Err(GraphError::Interrupted) => {}
        _ => panic!("Expected Interrupted error, got {:?}", result),
    }

    let checkpoint = checkpointer.load("thread-2").await.unwrap().unwrap();
    // Interrupted before B, blame node is usually "B" (current)
    assert_eq!(checkpoint.node, "B");
    // Queue must contain B
    assert!(checkpoint.queue.iter().any(|(n, _)| n == "B"));

    let resumed = graph
        .resume(checkpoint, ExecutionOptions::default())
        .await
        .unwrap();
    assert_eq!(resumed.data.executed, vec!["A", "B"]);
}

#[tokio::test]
async fn test_parallel_interrupt_resume() {
    let checkpointer = InMemoryCheckpointer::default();
    let graph = GraphBuilder::<DemoState>::new()
        .add_node(
            "A",
            RecordNode {
                name: "A".to_string(),
                delay: None,
            },
        )
        .add_node(
            "B",
            RecordNode {
                name: "B".to_string(),
                delay: Some(Duration::from_millis(10)),
            },
        )
        .add_node(
            "C",
            RecordNode {
                name: "C".to_string(),
                delay: Some(Duration::from_millis(100)),
            },
        )
        .add_edges("A", &["B", "C"])
        .set_entry("A")
        .with_checkpointer(checkpointer.clone(), "thread-3")
        .build();

    // Interrupt after B finishes. C should be running (slower).
    let options = ExecutionOptions {
        interrupt_after: vec!["B".to_string()],
        ..Default::default()
    };

    let result = graph
        .invoke_graph_with_options(GraphState::new(DemoState::default()), options)
        .await;

    match result {
        Err(GraphError::Interrupted) => {}
        _ => panic!("Expected Interrupted, got {:?}", result),
    }

    let checkpoint = checkpointer.load("thread-3").await.unwrap().unwrap();
    // Checkpoint should be from B
    assert_eq!(checkpoint.node, "B");

    // Queue should contain:
    // 1. Successors of B (none in this graph)
    // 2. Active tasks that were aborted (C)
    let queue_names: Vec<String> = checkpoint.queue.iter().map(|(n, _)| n.clone()).collect();
    assert!(
        queue_names.contains(&"C".to_string()),
        "Queue should contain aborted node C"
    );

    // Resume
    let resumed = graph
        .resume(checkpoint, ExecutionOptions::default())
        .await
        .unwrap();

    // Result should contain A, B, and C
    // Note: merging order depends on completion.
    // A, B (from first run). C (from resume).
    // StateSchema appends.
    // So ["A", "B", "C"] (unordered B/C? No, B finished first).
    let executed = resumed.data.executed;
    assert!(executed.contains(&"A".to_string()));
    assert!(executed.contains(&"B".to_string()));
    assert!(executed.contains(&"C".to_string()));
    assert_eq!(executed.len(), 3);
}
