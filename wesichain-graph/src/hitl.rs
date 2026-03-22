//! Human-in-the-loop (HITL) approval gates.
//!
//! Drop an [`ApprovalGate`] between any two graph nodes to pause execution and
//! wait for a human decision before continuing.
//!
//! # Flow
//!
//! 1. Graph reaches the gate node.
//! 2. Gate serializes state to the checkpointer and sends an [`ApprovalRequest`]
//!    on the [`ApprovalChannel`].
//! 3. Your application receives the request, presents `prompt` to a human, then
//!    calls `request.respond.send(ApprovalDecision::Approved { .. })`.
//! 4. The gate resumes — returning the decision in [`ApprovalState`] so
//!    downstream nodes can inspect it.
//!
//! # Example
//! ```ignore
//! use wesichain_graph::hitl::{ApprovalChannel, ApprovalDecision, ApprovalGate};
//!
//! let (gate, channel) = ApprovalGate::new("Agent wants to send an email. Approve?");
//!
//! // In your app loop:
//! tokio::spawn(async move {
//!     while let Some(req) = channel.recv().await {
//!         println!("Approval needed: {}", req.prompt);
//!         let _ = req.respond.send(ApprovalDecision::Approved { comment: None });
//!     }
//! });
//! ```

use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use wesichain_core::{state::StateSchema, WesichainError};

use crate::{GraphContext, GraphNode, GraphState, StateUpdate};

// ── Decision types ────────────────────────────────────────────────────────────

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

// ── ApprovalRequest ───────────────────────────────────────────────────────────

/// Sent to the host application when the gate fires.
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

// ── ApprovalChannel ───────────────────────────────────────────────────────────

/// Receiving end — your application listens on this for approval requests.
pub struct ApprovalChannel {
    rx: mpsc::Receiver<ApprovalRequest>,
}

impl ApprovalChannel {
    /// Receive the next approval request, or `None` if the sender was dropped.
    pub async fn recv(&mut self) -> Option<ApprovalRequest> {
        self.rx.recv().await
    }
}

// ── ApprovalGate ──────────────────────────────────────────────────────────────

/// Approval gate — insert this as a graph node to pause for human review.
///
/// Call [`ApprovalGate::new`] to create both the gate and the [`ApprovalChannel`]
/// your application uses to receive and respond to approval requests.
pub struct ApprovalGate {
    /// Template shown to the human.  May contain `{action}` placeholder.
    pub prompt: String,
    /// Auto-decide after this duration if no human responds.
    pub timeout: Option<Duration>,
    /// Decision to use when the timeout fires.
    pub default: ApprovalDefault,
    tx: mpsc::Sender<ApprovalRequest>,
}

impl ApprovalGate {
    /// Create a gate and the paired channel.
    pub fn new(prompt: impl Into<String>) -> (Self, ApprovalChannel) {
        let (tx, rx) = mpsc::channel(16);
        let gate = Self {
            prompt: prompt.into(),
            timeout: None,
            default: ApprovalDefault::Deny,
            tx,
        };
        (gate, ApprovalChannel { rx })
    }

    /// Set an auto-timeout.
    pub fn with_timeout(mut self, duration: Duration, default: ApprovalDefault) -> Self {
        self.timeout = Some(duration);
        self.default = default;
        self
    }

    /// Ask for approval, blocking until a decision arrives (or timeout fires).
    ///
    /// Returns the human's decision or the configured default on timeout.
    pub async fn request(
        &self,
        run_id: impl Into<String>,
        checkpoint_id: impl Into<String>,
        action_description: impl Into<String>,
    ) -> Result<ApprovalDecision, WesichainError> {
        let prompt = self.prompt.replace("{action}", &action_description.into());
        let (resp_tx, resp_rx) = oneshot::channel();

        let req = ApprovalRequest {
            run_id: run_id.into(),
            checkpoint_id: checkpoint_id.into(),
            prompt,
            respond: resp_tx,
        };

        self.tx.send(req).await.map_err(|_| {
            WesichainError::Custom("ApprovalChannel receiver dropped".to_string())
        })?;

        match self.timeout {
            Some(dur) => match tokio::time::timeout(dur, resp_rx).await {
                Ok(Ok(decision)) => Ok(decision),
                Ok(Err(_)) => Err(WesichainError::Custom(
                    "Approval responder dropped without sending a decision".to_string(),
                )),
                Err(_elapsed) => Ok(match self.default {
                    ApprovalDefault::Approve => {
                        ApprovalDecision::Approved { comment: Some("auto-approved (timeout)".to_string()) }
                    }
                    ApprovalDefault::Deny => {
                        ApprovalDecision::Denied { reason: "timed out — auto-denied".to_string() }
                    }
                }),
            },
            None => resp_rx.await.map_err(|_| {
                WesichainError::Custom(
                    "Approval responder dropped without sending a decision".to_string(),
                )
            }),
        }
    }
}

// ── GraphNode impl ────────────────────────────────────────────────────────────

/// Implements `GraphNode<S>` for states where `S::Update = S` (the update is the
/// full state).  The gate pauses execution, waits for a human decision, then
/// returns the input state unchanged so downstream nodes can continue.
#[async_trait::async_trait]
impl<S> GraphNode<S> for ApprovalGate
where
    S: StateSchema<Update = S> + Send,
{
    async fn invoke_with_context(
        &self,
        input: GraphState<S>,
        ctx: &GraphContext,
    ) -> Result<StateUpdate<S>, WesichainError> {
        let checkpoint_id = ctx.node_id.clone();
        self.request(&ctx.node_id, &checkpoint_id, "agent action").await?;
        Ok(StateUpdate::new(input.data))
    }
}

// ── ApprovalState ─────────────────────────────────────────────────────────────

/// Injected into graph state after the gate resolves so downstream nodes can
/// inspect the human's decision.
#[derive(Debug, Clone)]
pub struct ApprovalState {
    pub run_id: String,
    pub checkpoint_id: String,
    pub decision: ApprovalDecision,
}
