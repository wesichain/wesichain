#![cfg(feature = "google")]

use futures::StreamExt;
use httpmock::prelude::*;
use serde_json::json;
use wesichain_core::{Runnable, StreamEvent};
use wesichain_llm::{GoogleClient, LlmRequest, Message, Role};

#[tokio::test]
async fn google_stream_emits_text_chunks_and_final_answer() {
    let server = MockServer::start();
    let body = concat!(
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hel\"}]}}]}\n\n",
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"lo\"}]}}]}\n\n",
        "data: [DONE]\n\n"
    );

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1beta/models/gemini-1.5-flash:streamGenerateContent")
            .query_param("key", "test-key")
            .json_body(json!({
                "contents": [
                    {
                        "role": "user",
                        "parts": [{"text": "hi"}]
                    }
                ]
            }));
        then.status(200)
            .header("content-type", "text/event-stream")
            .body(body);
    });

    let client = GoogleClient::new("test-key", "gemini-1.5-flash").with_base_url(server.url(""));
    let request = LlmRequest {
        model: "".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "hi".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };

    let events: Vec<_> = client.stream(request).collect().await;
    assert!(matches!(events[0], Ok(StreamEvent::ContentChunk(ref text)) if text == "Hel"));
    assert!(matches!(events[1], Ok(StreamEvent::ContentChunk(ref text)) if text == "lo"));
    assert!(matches!(events[2], Ok(StreamEvent::FinalAnswer(ref text)) if text == "Hello"));
    mock.assert();
}

#[tokio::test]
async fn google_stream_detects_late_function_call() {
    let server = MockServer::start();
    let body = concat!(
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Calling tool\"}]}}]}\n\n",
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"calculator\",\"args\":{\"expression\":\"2+2\"}}}]}}]}\n\n",
        "data: [DONE]\n\n"
    );

    server.mock(|when, then| {
        when.method(POST)
            .path("/v1beta/models/gemini-1.5-flash:streamGenerateContent")
            .query_param("key", "test-key");
        then.status(200)
            .header("content-type", "text/event-stream")
            .body(body);
    });

    let client = GoogleClient::new("test-key", "gemini-1.5-flash").with_base_url(server.url(""));
    let request = LlmRequest {
        model: "".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "what is 2+2?".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };

    let events: Vec<_> = client.stream(request).collect().await;

    assert!(events.iter().any(
        |event| matches!(event, Ok(StreamEvent::ToolCallStart { name, .. }) if name == "calculator")
    ));
    assert!(events.iter().any(|event| {
        matches!(event, Ok(StreamEvent::ToolCallDelta { delta, .. }) if delta == &json!({"expression": "2+2"}))
    }));
}

#[tokio::test]
async fn google_stream_surfaces_http_errors() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1beta/models/gemini-1.5-flash:streamGenerateContent")
            .query_param("key", "test-key");
        then.status(429).json_body(json!({
            "error": {
                "message": "quota exceeded"
            }
        }));
    });

    let client = GoogleClient::new("test-key", "gemini-1.5-flash").with_base_url(server.url(""));
    let request = LlmRequest {
        model: "".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "hi".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };

    let mut events = client.stream(request);
    let first = events.next().await.expect("expected first event");
    assert!(
        matches!(first, Err(wesichain_core::WesichainError::LlmProvider(message)) if message.contains("quota exceeded"))
    );
    assert!(events.next().await.is_none());
}

#[tokio::test]
async fn google_stream_stops_after_parse_error() {
    let server = MockServer::start();
    let body = concat!(
        "data: {bad json}\n\n",
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"later\"}]}}]}\n\n",
        "data: [DONE]\n\n"
    );

    server.mock(|when, then| {
        when.method(POST)
            .path("/v1beta/models/gemini-1.5-flash:streamGenerateContent")
            .query_param("key", "test-key");
        then.status(200)
            .header("content-type", "text/event-stream")
            .body(body);
    });

    let client = GoogleClient::new("test-key", "gemini-1.5-flash").with_base_url(server.url(""));
    let request = LlmRequest {
        model: "".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "hi".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };

    let mut events = client.stream(request);
    let first = events.next().await.expect("expected first event");
    assert!(matches!(
        first,
        Err(wesichain_core::WesichainError::ParseFailed { .. })
    ));
    assert!(events.next().await.is_none());
}
