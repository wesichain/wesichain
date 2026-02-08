use serde_json::json;
use wesichain_core::AgentEvent;

#[test]
fn agent_event_serializes_with_tagged_shape() {
    let event = AgentEvent::Status {
        stage: "retrieving".to_string(),
        message: "Loading relevant chunks".to_string(),
        step: 1,
        thread_id: "thread-123".to_string(),
    };

    let value = serde_json::to_value(&event).expect("status event should serialize");

    assert_eq!(
        value,
        json!({
            "type": "Status",
            "data": {
                "stage": "retrieving",
                "message": "Loading relevant chunks",
                "step": 1,
                "thread_id": "thread-123"
            }
        })
    );
}

#[test]
fn agent_event_roundtrip_preserves_all_fields() {
    let event = AgentEvent::Thought {
        content: "I should call retrieval first".to_string(),
        step: 2,
        metadata: Some(json!({"confidence": 0.88})),
    };

    let encoded = serde_json::to_string(&event).expect("thought event should serialize");
    let decoded: AgentEvent = serde_json::from_str(&encoded).expect("thought event should decode");

    assert_eq!(decoded, event);
}

#[test]
fn agent_event_step_accessor_returns_expected_values() {
    let status = AgentEvent::Status {
        stage: "thinking".to_string(),
        message: "Planning next action".to_string(),
        step: 3,
        thread_id: "thread-a".to_string(),
    };
    let metadata = AgentEvent::Metadata {
        key: "model".to_string(),
        value: json!("gpt-4o-mini"),
    };

    assert_eq!(status.step(), Some(3));
    assert_eq!(metadata.step(), None);
}
