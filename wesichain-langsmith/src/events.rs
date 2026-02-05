use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

pub trait LangSmithInputs: Serialize {
    fn langsmith_inputs(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

pub trait LangSmithOutputs: Serialize {
    fn langsmith_outputs(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

impl LangSmithInputs for Value {
    fn langsmith_inputs(&self) -> Value {
        self.clone()
    }
}

impl LangSmithOutputs for Value {
    fn langsmith_outputs(&self) -> Value {
        self.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunType {
    Chain,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug)]
pub enum RunEvent {
    Start {
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        name: String,
        run_type: RunType,
        start_time: DateTime<Utc>,
        inputs: Value,
    },
    Update {
        run_id: Uuid,
        end_time: Option<DateTime<Utc>>,
        outputs: Option<Value>,
        error: Option<String>,
        duration_ms: Option<u128>,
    },
}
