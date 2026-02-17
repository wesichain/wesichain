use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use httpmock::prelude::*;
use serde_json::{json, Value as JsonValue};
use wesichain_core::{Document, StoreError, Value, VectorStore};
use wesichain_weaviate::WeaviateVectorStore;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn build_doc(id: &str, content: &str, embedding: Vec<f32>, metadata: JsonValue) -> Document {
    let metadata = metadata
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect::<HashMap<String, Value>>();

    Document {
        id: id.to_string(),
        content: content.to_string(),
        metadata,
        embedding: Some(embedding),
    }
}

fn unique_suffix() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{now}-{count}")
}

fn weaviate_contract_enabled() -> bool {
    std::env::var("RUN_WEAVIATE_CONTRACT").ok().as_deref() == Some("1")
}

fn weaviate_url() -> String {
    std::env::var("WEAVIATE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

fn weaviate_api_key() -> Option<String> {
    std::env::var("WEAVIATE_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn build_store(base_url: &str, class_name: &str, auto_create_class: bool) -> WeaviateVectorStore {
    let builder = WeaviateVectorStore::builder()
        .base_url(base_url)
        .class_name(class_name)
        .auto_create_class(auto_create_class);

    let builder = match weaviate_api_key() {
        Some(api_key) => builder.api_key(api_key),
        None => builder,
    };

    builder.build().expect("store should build")
}

#[tokio::test]
async fn contract_add_search_delete_roundtrip() {
    if !weaviate_contract_enabled() {
        return;
    }

    let class_name = format!("ContractRoundtrip{}", unique_suffix().replace('-', ""));
    let store = build_store(&weaviate_url(), &class_name, true);
    let docs = vec![
        build_doc(
            "doc-1",
            "alpha",
            vec![0.99, 0.01, 0.0],
            json!({"source": "contract", "rank": 1}),
        ),
        build_doc(
            "doc-2",
            "beta",
            vec![0.6, 0.4, 0.0],
            json!({"source": "contract", "rank": 2}),
        ),
    ];

    store.add(docs).await.expect("add should succeed");

    let results = store
        .search(&[1.0, 0.0, 0.0], 2, None)
        .await
        .expect("search should succeed");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].document.id, "doc-1");
    assert!(
        results
            .windows(2)
            .all(|pair| pair[0].score >= pair[1].score),
        "scores should be sorted descending"
    );
    assert_eq!(
        results[0].document.metadata.get("source"),
        Some(&json!("contract"))
    );

    store
        .delete(&["doc-1".to_string(), "doc-2".to_string()])
        .await
        .expect("delete should succeed");

    let after_delete = store
        .search(&[1.0, 0.0, 0.0], 2, None)
        .await
        .expect("search after delete should succeed");
    assert!(after_delete.is_empty());
}

#[tokio::test]
async fn contract_class_not_found_returns_clear_error() {
    let server = MockServer::start();
    let class_name = "MissingDoc";
    let store = build_store(&server.base_url(), class_name, false);

    let graphql_mock = server.mock(|when, then| {
        when.method(POST).path("/v1/graphql");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "errors": [
                    {"message": format!("class '{class_name}' not found in schema")}
                ]
            }));
    });

    let err = store
        .search(&[1.0, 0.0, 0.0], 1, None)
        .await
        .expect_err("search should fail for missing class");

    graphql_mock.assert();

    match err {
        StoreError::Internal(inner) => {
            let message = inner.to_string().to_lowercase();
            assert!(
                message.contains("class") && message.contains("not found"),
                "error should mention missing class, got: {message}"
            );
        }
        other => panic!("expected internal store error, got: {other:?}"),
    }
}

#[tokio::test]
async fn contract_class_auto_creation_bootstrap_behavior() {
    let server = MockServer::start();
    let class_name = "BootstrapDoc";
    let store = build_store(&server.base_url(), class_name, true);

    let mut first_add = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/objects")
            .body_contains("\"id\":\"doc-1\"");
        then.status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": [{"message": format!("class '{class_name}' not found") }]
            }));
    });

    let create_schema = server.mock(|when, then| {
        when.method(POST).path("/v1/schema");
        then.status(409)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": [{"message": format!("class '{class_name}' already exists") }]
            }))
            .delay(Duration::from_millis(150));
    });

    let doc = build_doc(
        "doc-1",
        "alpha",
        vec![1.0, 0.0, 0.0],
        json!({"source": "bootstrap"}),
    );

    let add_handle = tokio::spawn({
        let store = store.clone();
        async move { store.add(vec![doc]).await }
    });

    let mut retry_add = None;
    let mut first_add_seen = false;
    for _ in 0..100 {
        if first_add.hits() == 1 {
            first_add_seen = true;
            first_add.delete();
            retry_add = Some(server.mock(|when, then| {
                when.method(POST)
                    .path("/v1/objects")
                    .body_contains("\"id\":\"doc-1\"");
                then.status(200)
                    .header("content-type", "application/json")
                    .json_body(json!({"id": "doc-1"}));
            }));
            break;
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    assert!(
        retry_add.is_some(),
        "retry mock was not configured before timeout"
    );

    add_handle
        .await
        .expect("add task should join")
        .expect("add should succeed after class bootstrap");

    assert!(first_add_seen, "first add request should be observed once");
    assert_eq!(create_schema.hits(), 1);
    retry_add
        .expect("retry mock should be configured")
        .assert_hits(1);
}

#[tokio::test]
async fn contract_delete_allows_204_with_empty_body() {
    let server = MockServer::start();
    let class_name = "Doc";
    let store = build_store(&server.base_url(), class_name, false);

    let delete_doc_1 = server.mock(|when, then| {
        when.method(DELETE).path("/v1/objects/Doc/doc-1");
        then.status(204);
    });

    store
        .delete(&["doc-1".to_string()])
        .await
        .expect("delete should succeed on empty body 204 response");

    delete_doc_1.assert();
}

#[tokio::test]
async fn contract_delete_percent_encodes_document_id_path_segment() {
    let server = MockServer::start();
    let class_name = "Doc";
    let store = build_store(&server.base_url(), class_name, false);

    let delete_doc = server.mock(|when, then| {
        when.method(DELETE)
            .path("/v1/objects/Doc/doc%2Fwith%20space%3Fx%3D1");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({}));
    });

    store
        .delete(&["doc/with space?x=1".to_string()])
        .await
        .expect("delete should percent-encode id path segment");

    delete_doc.assert();
}

#[tokio::test]
async fn contract_add_search_delete_roundtrip_with_httpmock() {
    let server = MockServer::start();
    let class_name = "Doc";
    let store = build_store(&server.base_url(), class_name, false);

    let add_doc_1 = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/objects")
            .body_contains("\"id\":\"doc-1\"")
            .body_contains("\"class\":\"Doc\"");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({"id": "doc-1"}));
    });

    let add_doc_2 = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/objects")
            .body_contains("\"id\":\"doc-2\"")
            .body_contains("\"class\":\"Doc\"");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({"id": "doc-2"}));
    });

    let search = server.mock(|when, then| {
        when.method(POST).path("/v1/graphql");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "data": {
                    "Get": {
                        "Doc": [
                            {
                                "_additional": {"id": "doc-2", "certainty": 0.31},
                                "__wesichain_content": "beta",
                                "__wesichain_metadata": "{\"source\":\"contract\",\"rank\":2}"
                            },
                            {
                                "_additional": {"id": "doc-1", "certainty": 0.98},
                                "__wesichain_content": "alpha",
                                "__wesichain_metadata": "{\"source\":\"contract\",\"rank\":1}"
                            }
                        ]
                    }
                }
            }));
    });

    let delete_doc_1 = server.mock(|when, then| {
        when.method(DELETE).path("/v1/objects/Doc/doc-1");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({}));
    });

    let delete_doc_2 = server.mock(|when, then| {
        when.method(DELETE).path("/v1/objects/Doc/doc-2");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({}));
    });

    let docs = vec![
        build_doc(
            "doc-1",
            "alpha",
            vec![0.99, 0.01, 0.0],
            json!({"source": "contract", "rank": 1}),
        ),
        build_doc(
            "doc-2",
            "beta",
            vec![0.6, 0.4, 0.0],
            json!({"source": "contract", "rank": 2}),
        ),
    ];

    store.add(docs).await.expect("add should succeed");

    let results = store
        .search(&[1.0, 0.0, 0.0], 2, None)
        .await
        .expect("search should succeed");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].document.id, "doc-1");
    assert_eq!(results[0].document.content, "alpha");
    assert!(
        results
            .windows(2)
            .all(|pair| pair[0].score >= pair[1].score),
        "scores should be sorted descending"
    );
    assert_eq!(
        results[0].document.metadata.get("source"),
        Some(&json!("contract"))
    );

    store
        .delete(&["doc-1".to_string(), "doc-2".to_string()])
        .await
        .expect("delete should succeed");

    add_doc_1.assert();
    add_doc_2.assert();
    search.assert();
    delete_doc_1.assert();
    delete_doc_2.assert();
}
