//! Tests for ApprovalGate (HITL).

use std::time::Duration;

use wesichain_graph::hitl::{ApprovalDecision, ApprovalDefault, ApprovalGate};

// ── Basic approval flow ───────────────────────────────────────────────────────

#[tokio::test]
async fn gate_sends_request_and_resolves() {
    let (gate, mut channel) = ApprovalGate::new("Approve action: {action}");

    // Spawn a task that auto-approves the first request.
    tokio::spawn(async move {
        if let Some(req) = channel.recv().await {
            let _ = req.respond.send(ApprovalDecision::Approved { comment: None });
        }
    });

    let decision = gate.request("run-1", "ckpt-1", "delete file").await.unwrap();
    assert!(
        matches!(decision, ApprovalDecision::Approved { .. }),
        "expected Approved, got: {decision:?}"
    );
}

#[tokio::test]
async fn gate_timeout_auto_denies() {
    let (gate, _channel) = ApprovalGate::new("Approve?");
    let gate = gate.with_timeout(Duration::from_millis(10), ApprovalDefault::Deny);

    // No responder — gate should time out and auto-deny.
    let decision = gate.request("run-2", "ckpt-2", "action").await.unwrap();
    assert!(
        matches!(decision, ApprovalDecision::Denied { .. }),
        "expected Denied on timeout, got: {decision:?}"
    );
}

#[tokio::test]
async fn gate_timeout_auto_approves() {
    let (gate, _channel) = ApprovalGate::new("Approve?");
    let gate = gate.with_timeout(Duration::from_millis(10), ApprovalDefault::Approve);

    let decision = gate.request("run-3", "ckpt-3", "action").await.unwrap();
    assert!(
        matches!(decision, ApprovalDecision::Approved { .. }),
        "expected Approved on timeout, got: {decision:?}"
    );
}

// ── GraphNode impl ────────────────────────────────────────────────────────────

#[tokio::test]
async fn gate_as_graph_node() {
    use serde::{Deserialize, Serialize};
    use wesichain_graph::{GraphContext, GraphNode, GraphState, StateSchema, StateUpdate};
    use wesichain_graph::hitl::ApprovalGate;

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    struct SimpleState {
        value: i32,
    }

    impl StateSchema for SimpleState {
        type Update = SimpleState;
        fn apply(_current: &Self, update: Self::Update) -> Self {
            update
        }
    }

    let (gate, mut channel) = ApprovalGate::new("Approve?");

    // Auto-approve in background.
    tokio::spawn(async move {
        if let Some(req) = channel.recv().await {
            let _ = req.respond.send(ApprovalDecision::Approved { comment: None });
        }
    });

    let ctx = GraphContext {
        remaining_steps: None,
        observer: None,
        node_id: "gate-node".to_string(),
    };
    let input = GraphState::new(SimpleState { value: 42 });
    let update: StateUpdate<SimpleState> = gate.invoke_with_context(input, &ctx).await.unwrap();

    assert_eq!(update.data.value, 42, "gate should pass state through unchanged");
}
