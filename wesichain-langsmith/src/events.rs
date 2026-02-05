use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunType {
    Chain,
    Tool,
    Llm,
    Agent,
    Graph,
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
        trace_id: Uuid,
        name: String,
        run_type: RunType,
        start_time: DateTime<Utc>,
        inputs: Value,
        tags: Vec<String>,
        metadata: Value,
        session_name: String,
    },
    Update {
        run_id: Uuid,
        end_time: Option<DateTime<Utc>>,
        outputs: Option<Value>,
        error: Option<String>,
        duration_ms: Option<u128>,
    },
}
