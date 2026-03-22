//! Session data types.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use wesichain_core::Message;

/// A recorded tool call within a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool_name: String,
    pub args: Value,
    pub result: Value,
    pub timestamp_ms: u64,
}

/// A persistent conversation session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub messages: Vec<Message>,
    pub tool_history: Vec<ToolCallRecord>,
    pub metadata: std::collections::HashMap<String, Value>,
}

impl Session {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            messages: Vec::new(),
            tool_history: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Total character count across all message content (proxy for token usage).
    pub fn total_chars(&self) -> usize {
        self.messages.iter().map(|m| m.content.to_string().len()).sum()
    }
}
