//! Agent checkpoint and resume support.
//!
//! An [`AgentCheckpoint`] is a serializable snapshot of an agent run taken at
//! the moment it transitions into the `Interrupted` phase.  The host can
//! persist the checkpoint (file, database, `wesichain-session` store) and
//! later call [`AgentRuntime::resume_from`] to recreate the runtime state and
//! continue from where it left off.
//!
//! # Example
//! ```ignore
//! use wesichain_agent::{AgentCheckpoint, AgentRuntime};
//! use wesichain_agent::phase::{Idle, Interrupted};
//!
//! // Imagine the runtime reached Interrupted after being cancelled.
//! // In a real agent loop you'd have the messages that were in-flight.
//! let runtime: AgentRuntime<(), (), wesichain_agent::policy::NoopPolicy, Interrupted> =
//!     AgentRuntime::with_budget(10).think().interrupt();
//!
//! let checkpoint = runtime.checkpoint(vec![], 3);
//! let json = serde_json::to_string(&checkpoint).unwrap();
//!
//! // Later — deserialize and resume.
//! let loaded: AgentCheckpoint = serde_json::from_str(&json).unwrap();
//! let (resumed, messages, step_id) =
//!     AgentRuntime::<(), (), wesichain_agent::policy::NoopPolicy, Idle>::resume_from(&loaded);
//! assert_eq!(resumed.remaining_budget(), 10);
//! assert_eq!(step_id, 3);
//! ```

use serde::{Deserialize, Serialize};
use wesichain_core::Message;

/// A serializable snapshot of an interrupted agent run.
///
/// Stores enough state to reconstruct an `AgentRuntime` and resume the agent
/// loop at the exact step where it was interrupted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCheckpoint {
    /// Unique ID for this checkpoint (generated on creation).
    pub id: String,
    /// Unix timestamp (seconds) when the checkpoint was taken.
    pub created_at: u64,
    /// Conversation history at the time of interruption.
    pub messages: Vec<Message>,
    /// The step that was in progress when the run was interrupted.
    pub step_id: u32,
    /// Remaining step budget at the time of interruption.
    pub remaining_budget: u32,
}

impl AgentCheckpoint {
    /// Create a checkpoint from the caller-supplied conversation state.
    pub fn new(messages: Vec<Message>, step_id: u32, remaining_budget: u32) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self { id, created_at, messages, step_id, remaining_budget }
    }
}
