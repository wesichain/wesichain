use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value as JsonValue};
use wesichain_core::{Document, StoreError, Value, VectorStore};
use wesichain_qdrant::QdrantVectorStore;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn qdrant_contract_enabled() -> bool {
    std::env::var("RUN_QDRANT_CONTRACT").ok().as_deref() == Some("1")
}

fn unique_suffix() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{now}-{count}")
}

fn qdrant_url() -> String {
    std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://127.0.0.1:6333".to_string())
}

fn qdrant_api_key() -> Option<String> {
    std::env::var("QDRANT_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
}

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

fn build_store(collection: String) -> QdrantVectorStore {
    let builder = QdrantVectorStore::builder()
        .base_url(qdrant_url())
        .collection(collection);

    let builder = match qdrant_api_key() {
        Some(api_key) => builder.api_key(api_key),
        None => builder,
    };

    builder.build().expect("store should build")
}

async fn ensure_collection(
    base_url: &str,
    collection: &str,
    dimension: usize,
    api_key: Option<&str>,
) {
    let client = reqwest::Client::new();
    let endpoint = format!(
        "{}/collections/{}",
        base_url.trim_end_matches('/'),
        collection
    );

    let body = json!({
        "vectors": {
            "size": dimension,
            "distance": "Cosine"
        }
    });

    let mut request = client.put(endpoint).json(&body);
    if let Some(api_key) = api_key {
        request = request.header("api-key", api_key);
    }

    let response = request
        .send()
        .await
        .expect("collection create request should succeed");

    let status = response.status();
    let payload = response
        .text()
        .await
        .expect("collection create response should be readable");
    assert!(
        status.is_success(),
        "collection create should succeed, status={status}, body={payload}"
    );
}

#[tokio::test]
async fn contract_add_search_delete_roundtrip() {
    // Contract tests are opt-in because they require a live Qdrant instance.
    if !qdrant_contract_enabled() {
        return;
    }

    let collection = format!("contract_roundtrip_{}", unique_suffix());
    let api_key = qdrant_api_key();
    ensure_collection(&qdrant_url(), &collection, 3, api_key.as_deref()).await;

    let store = build_store(collection);
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
async fn contract_collection_not_found_returns_clear_error() {
    if !qdrant_contract_enabled() {
        return;
    }

    let missing_collection = format!("contract_missing_{}", unique_suffix());
    let store = build_store(missing_collection);

    let err = store
        .search(&[1.0, 0.0, 0.0], 1, None)
        .await
        .expect_err("search should fail for missing collection");

    match err {
        StoreError::Internal(inner) => {
            let message = inner.to_string().to_lowercase();
            assert!(
                message.contains("collection") && message.contains("not found"),
                "error should mention missing collection, got: {message}"
            );
        }
        other => panic!("expected internal store error, got: {other:?}"),
    }
}
