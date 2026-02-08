use serde_json::Value;
use wesichain_core::AgentEvent;
use wesichain_rag::adapters::sse::{done_event, ping_event, to_sse_event};

fn parse_sse(frame: &str) -> (&str, Value) {
    let mut event_type = "";
    let mut data = None;

    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("event: ") {
            event_type = rest;
        }
        if let Some(rest) = line.strip_prefix("data: ") {
            data = Some(serde_json::from_str(rest).expect("SSE data should be valid JSON"));
        }
    }

    (event_type, data.expect("SSE frame should include data"))
}

#[test]
fn sse_maps_status_to_status_event() {
    let event = AgentEvent::Status {
        stage: "retrieving".to_string(),
        message: "Running vector search".to_string(),
        step: 1,
        thread_id: "thread-1".to_string(),
    };

    let frame = to_sse_event(&event);
    let (event_type, data) = parse_sse(&frame);

    assert_eq!(event_type, "status");
    assert_eq!(data["stage"], "retrieving");
    assert_eq!(data["message"], "Running vector search");
    assert_eq!(data["thread_id"], "thread-1");
}

#[test]
fn sse_maps_final_to_answer_event() {
    let event = AgentEvent::Final {
        content: "Wesichain supports resumable graphs".to_string(),
        step: 4,
    };

    let frame = to_sse_event(&event);
    let (event_type, data) = parse_sse(&frame);

    assert_eq!(event_type, "answer");
    assert_eq!(data["content"], "Wesichain supports resumable graphs");
    assert_eq!(data["step"], 4);
}

#[test]
fn sse_exposes_ping_and_done_frames() {
    let ping_frame = ping_event();
    let done_frame = done_event();

    let (ping_type, ping_data) = parse_sse(&ping_frame);
    let (done_type, done_data) = parse_sse(&done_frame);

    assert_eq!(ping_type, "ping");
    assert_eq!(ping_data, serde_json::json!({}));
    assert_eq!(done_type, "done");
    assert_eq!(done_data, serde_json::json!({}));
}
