use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use wesichain_core::{AgentEvent, Runnable, StreamEvent, WesichainError};
use wesichain_graph::{
    ExecutionOptions, GraphBuilder, GraphError, GraphState, StateSchema, StateUpdate,
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

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
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
async fn graph_emits_monotonic_status_events_with_thread_id() {
    let graph = GraphBuilder::new()
        .add_node("one", AddOne)
        .set_entry("one")
        .build();
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);

    let out = graph
        .invoke_graph_with_options(
            GraphState::new(DemoState { count: 1 }),
            ExecutionOptions {
                agent_event_sender: Some(tx),
                agent_event_thread_id: Some("thread-evt-1".to_string()),
                ..ExecutionOptions::default()
            },
        )
        .await
        .expect("graph should succeed");
    assert_eq!(out.data.count, 2);

    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event);
    }

    assert!(!events.is_empty(), "expected emitted AgentEvent values");

    let steps: Vec<usize> = events.iter().filter_map(AgentEvent::step).collect();
    assert!(!steps.is_empty(), "expected step-bearing events");
    assert!(
        steps.windows(2).all(|window| window[1] > window[0]),
        "steps must be strictly increasing"
    );

    let status_events: Vec<&AgentEvent> = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::Status { .. }))
        .collect();
    assert!(
        !status_events.is_empty(),
        "expected at least one status event for node lifecycle"
    );

    for event in status_events {
        match event {
            AgentEvent::Status { thread_id, .. } => assert_eq!(thread_id, "thread-evt-1"),
            _ => unreachable!("status_events filter guarantees this variant"),
        }
    }
}

#[tokio::test]
async fn graph_emits_terminal_error_event_for_missing_entry() {
    let graph = GraphBuilder::new().set_entry("missing").build();
    let (tx, mut rx) = tokio::sync::mpsc::channel(4);

    let error = graph
        .invoke_graph_with_options(
            GraphState::new(DemoState { count: 0 }),
            ExecutionOptions {
                agent_event_sender: Some(tx),
                agent_event_thread_id: Some("thread-evt-2".to_string()),
                ..ExecutionOptions::default()
            },
        )
        .await
        .expect_err("missing node should fail");
    assert!(matches!(error, GraphError::MissingNode { .. }));

    let emitted = rx.recv().await.expect("an error event should be emitted");
    match emitted {
        AgentEvent::Error {
            recoverable,
            source,
            step,
            ..
        } => {
            assert!(!recoverable);
            assert_eq!(source.as_deref(), Some("graph"));
            assert_eq!(step, 1);
        }
        other => panic!("expected AgentEvent::Error, got {other:?}"),
    }
}
