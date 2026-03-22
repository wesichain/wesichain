//! Approval types for human-in-the-loop (HITL) gates.
//!
//! These pure data types live in `wesichain-core` so that `wesichain-agent`
//! can reference them without depending on `wesichain-graph`, which would
//! create a circular dependency.
//!
//! `wesichain-graph` re-exports all of these from `wesichain_graph::hitl`.

use tokio::sync::{mpsc, oneshot};

/// A human's response to an approval request.
#[derive(Debug, Clone)]
pub enum ApprovalDecision {
    /// The human approved the action.
    Approved { comment: Option<String> },
    /// The human denied the action.
    Denied { reason: String },
    /// The human modified the agent's intended action.
    Modified { new_input: String },
}

/// Default action when no human responds within the timeout.
#[derive(Debug, Clone, Copy, Default)]
pub enum ApprovalDefault {
    #[default]
    Deny,
    Approve,
}

/// Sent to the host application when an approval gate fires.
pub struct ApprovalRequest {
    /// Unique identifier for this execution run.
    pub run_id: String,
    /// Opaque checkpoint identifier the host can use to resume.
    pub checkpoint_id: String,
    /// Human-readable prompt describing what the agent wants to do.
    pub prompt: String,
    /// Send your [`ApprovalDecision`] here to unblock the gate.
    pub respond: oneshot::Sender<ApprovalDecision>,
}

/// Receiving end — your application listens on this for approval requests.
pub struct ApprovalChannel {
    rx: mpsc::Receiver<ApprovalRequest>,
}

impl ApprovalChannel {
    pub fn new(rx: mpsc::Receiver<ApprovalRequest>) -> Self {
        Self { rx }
    }

    /// Receive the next approval request, or `None` if the sender was dropped.
    pub async fn recv(&mut self) -> Option<ApprovalRequest> {
        self.rx.recv().await
    }
}
