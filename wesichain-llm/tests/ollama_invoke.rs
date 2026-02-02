use httpmock::prelude::*;
use serde_json::json;
use wesichain_core::Runnable;
use wesichain_llm::{LlmRequest, Message, OllamaClient, Role};

#[tokio::test]
async fn ollama_invoke_maps_response() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/api/chat");
        then.status(200).json_body(json!({
            "message": {"content": "hello"},
            "done": true,
            "tool_calls": []
        }));
    });

    let client = OllamaClient::new(server.url(""), "llama3.1".to_string()).expect("client");
    let req = LlmRequest {
        model: "llama3.1".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "hi".to_string(),
            tool_call_id: None,
        }],
        tools: vec![],
    };

    let resp = client.invoke(req).await.expect("invoke");
    assert_eq!(resp.content, "hello");
    mock.assert();
}
