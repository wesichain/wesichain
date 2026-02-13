use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A serializable representation of a Runnable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SerializableRunnable {
    Chain {
        steps: Vec<SerializableRunnable>,
    },
    Parallel {
        steps: HashMap<String, SerializableRunnable>,
    },
    Fallbacks {
        primary: Box<SerializableRunnable>,
        fallbacks: Vec<SerializableRunnable>,
    },
    Llm {
        model: String,
        #[serde(default)]
        params: HashMap<String, Value>,
    },
    Parser {
        kind: String, // "str", "json", "structured"
        #[serde(default)]
        target_type: Option<String>,
    },
    Prompt {
        template: String,
        input_variables: Vec<String>,
    },
    Tool {
        name: String,
        description: Option<String>,
        schema: Option<Value>,
    },
    Passthrough,
}

impl SerializableRunnable {
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

// Conversion to actual Runnable trait objects is handled in `src/persistence.rs` via `reconstruct`.

