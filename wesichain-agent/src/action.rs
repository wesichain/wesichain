use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wesichain_core::{LlmRequest, Runnable};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentAction {
    pub tool: String,
    pub tool_input: Value,
    pub log: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentFinish {
    pub return_values: Value,
    pub log: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStep {
    Action(AgentAction),
    Finish(AgentFinish),
}

/// Trait for agents that parse LLM output into an AgentStep.
#[async_trait]
pub trait ActionAgent: Runnable<LlmRequest, AgentStep> + Send + Sync {}
