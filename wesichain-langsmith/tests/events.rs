use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use wesichain_langsmith::{LangSmithInputs, LangSmithOutputs, RunEvent, RunStatus, RunType};

#[derive(Clone, Debug, Serialize)]
struct DemoPayload {
    token: String,
}

impl LangSmithInputs for DemoPayload {}
impl LangSmithOutputs for DemoPayload {}

#[test]
fn langsmith_traits_use_serde_json() {
    let payload = DemoPayload {
        token: "secret".to_string(),
    };
    assert_eq!(payload.langsmith_inputs(), json!({"token": "secret"}));
    assert_eq!(payload.langsmith_outputs(), json!({"token": "secret"}));
}

#[test]
fn run_event_variants_capture_metadata() {
    let run_id = Uuid::new_v4();
    let start = RunEvent::Start {
        run_id,
        parent_run_id: None,
        trace_id: run_id,
        name: "node".to_string(),
        run_type: RunType::Chain,
        start_time: Utc::now(),
        inputs: json!({"value": 1}),
        tags: Vec::new(),
        metadata: json!({}),
        session_name: "test".to_string(),
    };

    let update = RunEvent::Update {
        run_id,
        end_time: Some(Utc::now()),
        outputs: Some(json!({"value": 2})),
        error: None,
        duration_ms: Some(10),
    };

    match start {
        RunEvent::Start { run_type, .. } => assert_eq!(run_type, RunType::Chain),
        _ => panic!("expected start event"),
    }
    match update {
        RunEvent::Update { error, .. } => assert_eq!(error, None),
        _ => panic!("expected update event"),
    }

    let status = RunStatus::Running;
    assert_eq!(status, RunStatus::Running);
}
