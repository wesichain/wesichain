use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Model transport")]
    ModelTransport,
    #[error("Invalid model action at step {step_id}")]
    InvalidModelAction {
        step_id: u32,
        tool_name: Option<String>,
        received_args: serde_json::Value,
        raw_response: serde_json::Value,
    },
    #[error("Tool dispatch")]
    ToolDispatch,
    #[error("Budget exceeded")]
    BudgetExceeded,
    #[error("Policy config invalid")]
    PolicyConfigInvalid,
    #[error("Policy runtime violation")]
    PolicyRuntimeViolation,
    #[error("Internal invariant")]
    InternalInvariant,
}
