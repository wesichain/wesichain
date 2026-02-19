mod error;
mod event;
mod llm;
mod phase;
mod policy;
mod runtime;
mod state;
mod tooling;
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
pub use state::AgentState;
pub use tooling::{
    CancellationToken, ToolCallEnvelope, ToolContext, ToolError, ToolSchema, ToolSet,
    ToolSetBuildError, TypedTool,
};
pub use validation::{validate_model_action, ModelAction};
