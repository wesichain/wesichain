use crate::tooling::ToolError;

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

#[derive(Debug)]
pub enum ToolDispatchError {
    UnknownTool {
        name: String,
        call_id: String,
    },
    InvalidArgs {
        name: String,
        call_id: String,
        source: serde_json::Error,
    },
    Execution {
        name: String,
        call_id: String,
        source: ToolError,
    },
    Serialization {
        name: String,
        call_id: String,
        source: serde_json::Error,
    },
}

impl std::fmt::Display for ToolDispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolDispatchError::UnknownTool { name, call_id } => {
                write!(f, "unknown tool {name} for call {call_id}")
            }
            ToolDispatchError::InvalidArgs { name, call_id, .. } => {
                write!(f, "invalid args for tool {name} in call {call_id}")
            }
            ToolDispatchError::Execution { name, call_id, .. } => {
                write!(f, "tool {name} failed during call {call_id}")
            }
            ToolDispatchError::Serialization { name, call_id, .. } => {
                write!(
                    f,
                    "failed to serialize output for tool {name} in call {call_id}"
                )
            }
        }
    }
}

impl std::error::Error for ToolDispatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ToolDispatchError::UnknownTool { .. } => None,
            ToolDispatchError::InvalidArgs { source, .. } => Some(source),
            ToolDispatchError::Execution { source, .. } => Some(source),
            ToolDispatchError::Serialization { source, .. } => Some(source),
        }
    }
}
