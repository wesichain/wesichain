use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use wesichain_core::{StoreError, VectorStore};
use wesichain_qdrant::{QdrantStoreError, QdrantVectorStore};

fn spawn_single_response_server(response_status: u16, response_body: &'static str) -> String {
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

        let status_line = match response_status {
            200 => "HTTP/1.1 200 OK",
            500 => "HTTP/1.1 500 Internal Server Error",
            other => panic!("unsupported status in test server: {other}"),
        };
        let response = format!(
            "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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
async fn search_surfaces_invalid_response_error_on_json_decode_failure() {
    let base_url = spawn_single_response_server(200, "not-json");
    let store = QdrantVectorStore::builder()
        .base_url(base_url)
        .collection("docs")
        .build()
        .expect("store should build");

    let err = store
        .search(&[0.1, 0.2, 0.3], 1, None)
        .await
        .expect_err("search should fail for invalid JSON body");

    match err {
        StoreError::Internal(inner) => {
            let qdrant_err = inner
                .downcast_ref::<QdrantStoreError>()
                .expect("error should be QdrantStoreError");
            assert!(matches!(
                qdrant_err,
                QdrantStoreError::InvalidResponse { .. }
            ));
        }
        other => panic!("expected internal store error, got: {other:?}"),
    }
}
