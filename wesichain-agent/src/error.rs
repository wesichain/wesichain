#[derive(Debug)]
pub enum AgentError {
    ModelTransport,
    InvalidModelAction {
        step_id: u32,
        tool_name: Option<String>,
        received_args: String,
        raw_response: String,
    },
    ToolDispatch,
    BudgetExceeded,
    PolicyConfigInvalid,
    PolicyRuntimeViolation,
    InternalInvariant,
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::ModelTransport => f.write_str("Model transport"),
            AgentError::InvalidModelAction { step_id, .. } => {
                write!(f, "Invalid model action at step {step_id}")
            }
            AgentError::ToolDispatch => f.write_str("Tool dispatch"),
            AgentError::BudgetExceeded => f.write_str("Budget exceeded"),
            AgentError::PolicyConfigInvalid => f.write_str("Policy config invalid"),
            AgentError::PolicyRuntimeViolation => f.write_str("Policy runtime violation"),
            AgentError::InternalInvariant => f.write_str("Internal invariant"),
        }
    }
}

impl std::error::Error for AgentError {}
