use futures::StreamExt;
use httpmock::prelude::*;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_llm::{LlmRequest, Message, OllamaClient, Role};

async fn spawn_chunked_server(chunks: Vec<&'static str>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut received = Vec::new();
        let mut buf = [0_u8; 1024];
        loop {
            let read = socket.read(&mut buf).await.expect("read");
            if read == 0 {
                break;
            }
            received.extend_from_slice(&buf[..read]);
            if received.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        let headers = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: application/x-ndjson\r\n",
            "Transfer-Encoding: chunked\r\n",
            "\r\n"
        );
        socket.write_all(headers.as_bytes()).await.expect("headers");
        for chunk in chunks {
            let len = chunk.len();
            socket
                .write_all(format!("{:X}\r\n", len).as_bytes())
                .await
                .expect("chunk len");
            socket
                .write_all(chunk.as_bytes())
                .await
                .expect("chunk data");
            socket.write_all(b"\r\n").await.expect("chunk end");
        }
        socket.write_all(b"0\r\n\r\n").await.expect("eof");
        let _ = socket.shutdown().await;
    });

    format!("http://{}", addr)
}

#[tokio::test]
async fn ollama_stream_emits_events() {
    let server = MockServer::start();
    let body = "{\"message\":{\"content\":\"Hel\"},\"done\":false}\n{\"message\":{\"content\":\"lo\"},\"done\":true}";
    server.mock(|when, then| {
        when.method(POST).path("/api/chat").json_body(json!({
            "model": "llama3.1",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [],
            "stream": true
        }));
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

    let events: Vec<_> = client.stream(req).collect().await;
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], Ok(StreamEvent::ContentChunk(ref content)) if content == "Hel"));
    assert!(matches!(events[1], Ok(StreamEvent::FinalAnswer(ref content)) if content == "lo"));
}

#[tokio::test]
async fn ollama_stream_surfaces_http_errors() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/api/chat");
        then.status(500)
            .body("{\"message\":{\"content\":\"bad\"},\"done\":true}");
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
    let first = events.next().await.expect("event").expect_err("err");
    assert!(matches!(first, WesichainError::LlmProvider(_)));
    assert!(events.next().await.is_none());
    mock.assert();
}

#[tokio::test]
async fn ollama_stream_stops_on_parse_error() {
    let base_url = spawn_chunked_server(vec![
        "{\"message\":{#}}\n",
        "{\"message\":{\"content\":\"ok\"},\"done\":true}\n",
    ])
    .await;

    let client = OllamaClient::new(base_url, "llama3.1".to_string()).expect("client");
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
    let first = events.next().await.expect("event").expect_err("err");
    assert!(matches!(first, WesichainError::ParseFailed { .. }));
    assert!(events.next().await.is_none());
}
