use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use wesichain_qdrant::QdrantVectorStore;

fn spawn_single_response_server(response_body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("get local addr");

    thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept socket");
        let mut request = Vec::new();
        let mut buf = [0_u8; 1024];

        loop {
            let read = socket.read(&mut buf).expect("read request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buf[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );

        socket
            .write_all(response.as_bytes())
            .expect("write response");
    });

    format!("http://{addr}")
}

#[tokio::test]
async fn scored_search_returns_results_sorted_descending_by_score() {
    let base_url = spawn_single_response_server(
        r#"{"result":[{"id":"doc-1","score":0.1,"payload":{"__wesichain_content":"first"}},{"id":"doc-2","score":0.9,"payload":{"__wesichain_content":"second"}},{"id":"doc-3","score":0.4,"payload":{"__wesichain_content":"third"}}]}"#,
    );

    let store = QdrantVectorStore::builder()
        .base_url(base_url)
        .collection("docs")
        .build()
        .expect("store should build");

    let results = store
        .scored_search(&[1.0, 0.0], 3, None)
        .await
        .expect("scored search should succeed");

    assert_eq!(results.len(), 3);
    assert!(results
        .windows(2)
        .all(|pair| pair[0].score >= pair[1].score));
    assert_eq!(results[0].document.id, "doc-2");
    assert_eq!(results[1].document.id, "doc-3");
    assert_eq!(results[2].document.id, "doc-1");
}
