use serde_json::json;
use wesichain_core::{LlmRequest, LlmResponse, Message, Role, ToolCall, ToolSpec};

#[test]
fn llm_types_serialize_with_tool_calls() {
    let call = ToolCall {
        id: "call-1".to_string(),
        name: "calculator".to_string(),
        args: json!({"expression": "2+2"}),
    };
    let message = Message {
        role: Role::Assistant,
        content: "".to_string(),
        tool_call_id: None,
        tool_calls: vec![call.clone()],
    };
    let req = LlmRequest {
        model: "test".to_string(),
        messages: vec![message],
        tools: vec![ToolSpec {
            name: "calculator".to_string(),
            description: "math".to_string(),
            parameters: json!({"type": "object"}),
        }],
    };
    let value = serde_json::to_value(req).expect("serialize request");
    assert!(value["messages"][0]["tool_calls"].is_array());

    let response = LlmResponse {
        content: "".to_string(),
        tool_calls: vec![call],
    };
    let response_value = serde_json::to_value(response).expect("serialize response");
    assert!(response_value["tool_calls"].is_array());
}
