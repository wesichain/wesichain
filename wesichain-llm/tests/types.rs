use serde_json::json;
use wesichain_llm::{LlmRequest, Message, Role, ToolSpec};

#[test]
fn llm_request_serializes_with_tools() {
    let req = LlmRequest {
        model: "llama3.1".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "hi".to_string(),
            tool_call_id: None,
        }],
        tools: vec![ToolSpec {
            name: "calculator".to_string(),
            description: "math".to_string(),
            parameters: json!({"type":"object","properties":{}}),
        }],
    };

    let value = serde_json::to_value(req).expect("serialize");
    assert_eq!(value["model"], "llama3.1");
    assert_eq!(value["messages"][0]["role"], "user");
    assert_eq!(value["tools"][0]["name"], "calculator");

    let tool_msg = Message {
        role: Role::Tool,
        content: "ok".to_string(),
        tool_call_id: Some("call-1".to_string()),
    };
    let tool_value = serde_json::to_value(tool_msg).expect("serialize tool msg");
    assert_eq!(tool_value["tool_call_id"], "call-1");
}
