use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AgentEvent {
    Status {
        stage: String,
        message: String,
        step: usize,
        thread_id: String,
    },
    Thought {
        content: String,
        step: usize,
        metadata: Option<serde_json::Value>,
    },
    ToolCall {
        id: String,
        tool_name: String,
        input: serde_json::Value,
        step: usize,
    },
    Observation {
        id: String,
        tool_name: String,
        output: serde_json::Value,
        step: usize,
    },
    Final {
        content: String,
        step: usize,
    },
    Error {
        message: String,
        step: usize,
        recoverable: bool,
        source: Option<String>,
    },
    Metadata {
        key: String,
        value: serde_json::Value,
    },
}

impl AgentEvent {
    pub fn step(&self) -> Option<usize> {
        match self {
            Self::Status { step, .. }
            | Self::Thought { step, .. }
            | Self::ToolCall { step, .. }
            | Self::Observation { step, .. }
            | Self::Final { step, .. }
            | Self::Error { step, .. } => Some(*step),
            Self::Metadata { .. } => None,
        }
    }

    pub fn thread_id(&self) -> Option<&str> {
        match self {
            Self::Status { thread_id, .. } => Some(thread_id.as_str()),
            _ => None,
        }
    }
}
