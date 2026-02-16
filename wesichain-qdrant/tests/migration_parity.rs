use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{json, Value as JsonValue};
use wesichain_core::Value;
use wesichain_core::VectorStore;
use wesichain_qdrant::QdrantVectorStore;

#[path = "../examples/rag_integration.rs"]
mod rag_integration;

#[derive(Default, Debug)]
struct RecordedRequests {
    upsert_body: Option<JsonValue>,
    delete_body: Option<JsonValue>,
}

fn parse_content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.starts_with("content-length:") {
                line.split(':')
                    .nth(1)
                    .and_then(|value| value.trim().parse::<usize>().ok())
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn read_http_request(socket: &mut std::net::TcpStream) -> (String, Vec<u8>) {
    let mut bytes = Vec::new();
    let mut buf = [0_u8; 1024];

    loop {
        let read = socket.read(&mut buf).expect("read request chunk");
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }

    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("http header terminator should exist")
        + 4;

    let headers = String::from_utf8(bytes[..header_end].to_vec()).expect("headers should be utf-8");
    let content_length = parse_content_length(&headers);

    while bytes.len() < header_end + content_length {
        let read = socket.read(&mut buf).expect("read request body chunk");
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..read]);
    }

    (
        headers,
        bytes[header_end..header_end + content_length].to_vec(),
    )
}

fn spawn_qdrant_core_flow_server() -> (String, Arc<Mutex<RecordedRequests>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("resolve local test address");
    let recorded = Arc::new(Mutex::new(RecordedRequests::default()));
    let recorded_for_thread = Arc::clone(&recorded);

    thread::spawn(move || {
        let mut search_count = 0;

        for _ in 0..4 {
            let (mut socket, _) = listener.accept().expect("accept socket");
            let (headers, body) = read_http_request(&mut socket);
            let request_line = headers.lines().next().expect("request line should exist");

            let response_body = if request_line
                .starts_with("PUT /collections/docs/points?wait=true")
            {
                let body_json: JsonValue = serde_json::from_slice(&body)
                    .expect("upsert request body should be valid json");
                recorded_for_thread
                    .lock()
                    .expect("lock recorded requests")
                    .upsert_body = Some(body_json);
                json!({"result": {"status": "acknowledged"}})
            } else if request_line.starts_with("POST /collections/docs/points/search") {
                search_count += 1;
                if search_count == 1 {
                    json!({
                        "result": [
                            {
                                "id": "doc-2",
                                "score": 0.44,
                                "payload": {
                                    "__wesichain_content": "Qdrant stores vectors and metadata for similarity retrieval.",
                                    "source": "guide"
                                }
                            },
                            {
                                "id": "doc-1",
                                "score": 0.93,
                                "payload": {
                                    "__wesichain_content": "Wesichain is a Rust-native LLM framework focused on graph and agent workflows.",
                                    "source": "guide"
                                }
                            }
                        ]
                    })
                } else {
                    json!({"result": []})
                }
            } else if request_line.starts_with("POST /collections/docs/points/delete?wait=true") {
                let body_json: JsonValue = serde_json::from_slice(&body)
                    .expect("delete request body should be valid json");
                recorded_for_thread
                    .lock()
                    .expect("lock recorded requests")
                    .delete_body = Some(body_json);
                json!({"result": {"status": "acknowledged"}})
            } else {
                panic!("unexpected request line: {request_line}");
            };

            let response_body_string = response_body.to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body_string.len(),
                response_body_string
            );
            socket
                .write_all(response.as_bytes())
                .expect("write response");
        }
    });

    (format!("http://{addr}"), recorded)
}

fn metadata_source(value: &HashMap<String, Value>) -> Option<&str> {
    value.get("source").and_then(|v| v.as_str())
}

#[tokio::test]
async fn migration_parity_core_flow_behaves_like_langchain_roundtrip() {
    let (base_url, recorded) = spawn_qdrant_core_flow_server();

    let store = QdrantVectorStore::builder()
        .base_url(base_url)
        .collection("docs")
        .build()
        .expect("store should build");

    let docs = rag_integration::sample_documents();
    let results = rag_integration::run_core_flow(&store, docs)
        .await
        .expect("core flow should succeed");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].document.id, "doc-1");
    assert_eq!(results[1].document.id, "doc-2");
    assert_eq!(
        metadata_source(&results[0].document.metadata),
        Some("guide")
    );

    let after_delete = store
        .search(&[0.98, 0.02, 0.0], 2, None)
        .await
        .expect("search after delete should succeed");
    assert!(after_delete.is_empty());

    let recorded = recorded.lock().expect("lock recorded requests");
    let upsert = recorded
        .upsert_body
        .as_ref()
        .expect("upsert request should be recorded");
    assert_eq!(upsert["points"][0]["vector"], json!([0.99, 0.01, 0.0]));
    assert_eq!(upsert["points"][1]["vector"], json!([0.70, 0.30, 0.0]));
    assert_eq!(
        upsert["points"][0]["payload"]["__wesichain_content"],
        json!("Wesichain is a Rust-native LLM framework focused on graph and agent workflows.")
    );

    let delete = recorded
        .delete_body
        .as_ref()
        .expect("delete request should be recorded");
    assert_eq!(delete["points"], json!(["doc-1", "doc-2"]));
}
