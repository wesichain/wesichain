use std::collections::HashMap;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{ToolCall, Value};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ReActStep {
    Thought(String),
    Action(ToolCall),
    Observation(Value),
    FinalAnswer(String),
    Error(String),
}

pub trait ScratchpadState: Serialize + DeserializeOwned {
    fn scratchpad(&self) -> &Vec<ReActStep>;
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep>;
    fn iteration_count(&self) -> u32;
    fn increment_iteration(&mut self);
    fn ensure_scratchpad(&mut self) {}
}

pub trait HasUserInput {
    fn user_input(&self) -> &str;
}

pub trait HasFinalOutput {
    fn final_output(&self) -> Option<&str>;
    fn set_final_output(&mut self, value: String);
}

impl HasUserInput for HashMap<String, Value> {
    fn user_input(&self) -> &str {
        self.get("input").and_then(|v| v.as_str()).unwrap_or("")
    }
}

impl HasFinalOutput for HashMap<String, Value> {
    fn final_output(&self) -> Option<&str> {
        self.get("final_output").and_then(|v| v.as_str())
    }

    fn set_final_output(&mut self, value: String) {
        self.insert("final_output".to_string(), Value::String(value));
    }
}
