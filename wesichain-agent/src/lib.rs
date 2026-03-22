pub mod as_tool;
pub mod checkpoint;
mod error;
mod event;
mod llm;
mod phase;
mod policy;
pub mod permission;
mod runtime;
mod state;
pub mod tooling;
mod validation;

pub use error::{AgentError, ToolDispatchError};
pub use event::{
    validate_completed_once, validate_step_started_precedes_terminal,
    validate_tool_dispatch_cardinality, AgentEvent,
};
pub use llm::LlmAdapter;
pub use phase::{Acting, Completed, Failed, Idle, Interrupted, Observing, Thinking};
pub use policy::{NoopPolicy, PolicyDecision, PolicyEngine, RepromptStrategy};
pub use runtime::{
    emit_single_step_events, emit_tool_step_events, AgentRuntime, LoopTransition,
    ToolDispatchOutcome,
};
pub use checkpoint::AgentCheckpoint;
pub use state::AgentState;
pub use as_tool::AgentAsTool;
pub use permission::{PermissionCheck, PermissionPolicy, ToolPermission};
pub use tooling::{
    CancellationToken, Tool, ToolCallEnvelope, ToolContext, ToolError, ToolSchema, ToolSet,
    ToolSetBuildError, TypedTool,
};
pub use validation::{validate_model_action, ModelAction};
