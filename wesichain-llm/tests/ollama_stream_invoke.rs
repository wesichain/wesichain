use futures::StreamExt;
use httpmock::prelude::*;
use wesichain_core::Runnable;
use wesichain_llm::{LlmRequest, Message, OllamaClient, Role};

#[tokio::test]
async fn ollama_stream_emits_events() {
    let server = MockServer::start();
    let body = "{\"message\":{\"content\":\"Hel\"},\"done\":false}\n{\"message\":{\"content\":\"lo\"},\"done\":true}";
    server.mock(|when, then| {
        when.method(POST).path("/api/chat");
        then.status(200)
            .body(body)
            .header("content-type", "application/x-ndjson");
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

    let mut events = client.stream(req);
    let first = events.next().await.expect("event").expect("ok");
    assert!(matches!(first, wesichain_core::StreamEvent::ContentChunk(_)));
}
