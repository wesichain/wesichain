use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{json, Value as JsonValue};
use wesichain_core::{Value, VectorStore};
use wesichain_weaviate::WeaviateVectorStore;

#[path = "../examples/rag_integration.rs"]
mod rag_integration;

#[derive(Default, Debug)]
struct RecordedRequests {
    objects: Vec<JsonValue>,
    graphql_queries: Vec<String>,
    delete_paths: Vec<String>,
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

fn spawn_weaviate_core_flow_server() -> (String, Arc<Mutex<RecordedRequests>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("resolve local test address");
    let recorded = Arc::new(Mutex::new(RecordedRequests::default()));
    let recorded_for_thread = Arc::clone(&recorded);

    thread::spawn(move || {
        let mut search_count = 0;

        for _ in 0..6 {
            let (mut socket, _) = listener.accept().expect("accept socket");
            let (headers, body) = read_http_request(&mut socket);
            let request_line = headers.lines().next().expect("request line should exist");

            let response_body = if request_line.starts_with("POST /v1/objects ") {
                let body_json: JsonValue =
                    serde_json::from_slice(&body).expect("object body should be valid json");
                recorded_for_thread
                    .lock()
                    .expect("lock recorded requests")
                    .objects
                    .push(body_json);
                json!({"id": "ok"})
            } else if request_line.starts_with("POST /v1/graphql ") {
                let body_json: JsonValue =
                    serde_json::from_slice(&body).expect("graphql body should be valid json");
                let query = body_json
                    .get("query")
                    .and_then(JsonValue::as_str)
                    .expect("graphql body should include string query")
                    .to_string();
                recorded_for_thread
                    .lock()
                    .expect("lock recorded requests")
                    .graphql_queries
                    .push(query);

                search_count += 1;
                if search_count == 1 {
                    json!({
                        "data": {
                            "Get": {
                                "Docs": [
                                    {
                                        "_additional": {"id": "doc-2", "certainty": 0.44},
                                        "__wesichain_content": "Weaviate stores vectors and metadata for similarity retrieval.",
                                        "__wesichain_metadata": "{\"source\":\"guide\"}"
                                    },
                                    {
                                        "_additional": {"id": "doc-1", "certainty": 0.93},
                                        "__wesichain_content": "Wesichain is a Rust-native LLM framework focused on graph and agent workflows.",
                                        "__wesichain_metadata": "{\"source\":\"guide\"}"
                                    }
                                ]
                            }
                        }
                    })
                } else {
                    json!({
                        "data": {
                            "Get": {
                                "Docs": []
                            }
                        }
                    })
                }
            } else if request_line.starts_with("DELETE /v1/objects/Docs/") {
                let path = request_line
                    .split_whitespace()
                    .nth(1)
                    .expect("request path should exist")
                    .to_string();
                recorded_for_thread
                    .lock()
                    .expect("lock recorded requests")
                    .delete_paths
                    .push(path);
                json!({})
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
    let (base_url, recorded) = spawn_weaviate_core_flow_server();

    let store = WeaviateVectorStore::builder()
        .base_url(base_url)
        .class_name("Docs")
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
    assert_eq!(recorded.objects.len(), 2);
    assert_eq!(recorded.objects[0]["class"], json!("Docs"));
    assert_eq!(recorded.objects[0]["id"], json!("doc-1"));
    assert_eq!(recorded.objects[0]["vector"], json!([0.99, 0.01, 0.0]));
    assert_eq!(
        recorded.objects[0]["properties"]["__wesichain_content"],
        json!("Wesichain is a Rust-native LLM framework focused on graph and agent workflows.")
    );
    assert_eq!(recorded.objects[1]["id"], json!("doc-2"));
    assert_eq!(recorded.objects[1]["vector"], json!([0.70, 0.30, 0.0]));

    assert_eq!(recorded.graphql_queries.len(), 2);
    assert!(recorded.graphql_queries[0].contains("Get{Docs("));
    assert!(
        recorded.graphql_queries[0].contains("nearVector:{vector:[0.98,0.02,0.0]}"),
        "search should query with expected embedding"
    );

    assert_eq!(
        recorded.delete_paths,
        vec![
            "/v1/objects/Docs/doc-1".to_string(),
            "/v1/objects/Docs/doc-2".to_string()
        ]
    );
}
