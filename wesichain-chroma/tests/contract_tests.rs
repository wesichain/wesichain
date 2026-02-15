use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use wesichain_chroma::ChromaVectorStore;
use wesichain_core::{Document, Value, VectorStore};
use wesichain_retrieval::InMemoryVectorStore;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_suffix() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{now}-{count}")
}

fn build_doc(
    id: &str,
    content: &str,
    embedding: Vec<f32>,
    metadata: serde_json::Value,
) -> Document {
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

fn chroma_contract_enabled() -> bool {
    std::env::var("RUN_CHROMA_CONTRACT").ok().as_deref() == Some("1")
}

async fn make_chroma_store(test_name: &str) -> Arc<dyn VectorStore> {
    let endpoint =
        std::env::var("CHROMA_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:8000".to_string());
    let collection_name = format!("contract_{test_name}_{}", unique_suffix());

    let store = ChromaVectorStore::new(endpoint, collection_name)
        .await
        .expect("chroma store should be creatable");

    Arc::new(store)
}

fn make_in_memory_store() -> Arc<dyn VectorStore> {
    Arc::new(InMemoryVectorStore::new())
}

async fn contract_add_single_vector(store: &dyn VectorStore) {
    store
        .add(vec![build_doc(
            "doc1",
            "hello",
            vec![1.0, 0.0],
            json!({"source": "test"}),
        )])
        .await
        .expect("add should succeed");

    let results = store
        .search(&[1.0, 0.0], 1, None)
        .await
        .expect("search should succeed");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document.id, "doc1");
}

async fn contract_search_returns_top_k(store: &dyn VectorStore) {
    let docs = vec![
        build_doc("doc1", "a", vec![1.0, 0.0], json!({"rank": 1})),
        build_doc("doc2", "b", vec![0.9, 0.1], json!({"rank": 2})),
        build_doc("doc3", "c", vec![0.8, 0.2], json!({"rank": 3})),
        build_doc("doc4", "d", vec![0.0, 1.0], json!({"rank": 4})),
        build_doc("doc5", "e", vec![-1.0, 0.0], json!({"rank": 5})),
    ];

    store.add(docs).await.expect("add should succeed");

    let results = store
        .search(&[1.0, 0.0], 3, None)
        .await
        .expect("search should succeed");

    assert_eq!(results.len(), 3);
    assert_eq!(results[0].document.id, "doc1");
    assert!(
        results
            .windows(2)
            .all(|pair| pair[0].score >= pair[1].score),
        "results must be score-sorted descending"
    );
}

async fn contract_delete_removes_vector(store: &dyn VectorStore) {
    store
        .add(vec![build_doc(
            "doc1",
            "to-delete",
            vec![1.0, 0.0],
            json!({}),
        )])
        .await
        .expect("add should succeed");

    store
        .delete(&["doc1".to_string()])
        .await
        .expect("delete should succeed");

    let results = store
        .search(&[1.0, 0.0], 5, None)
        .await
        .expect("search should succeed");

    assert!(results.is_empty());
}

async fn contract_duplicate_id_overwrites(store: &dyn VectorStore) {
    store
        .add(vec![build_doc(
            "doc1",
            "old",
            vec![1.0, 0.0],
            json!({"version": 1}),
        )])
        .await
        .expect("first add should succeed");

    store
        .add(vec![build_doc(
            "doc1",
            "new",
            vec![0.0, 1.0],
            json!({"version": 2}),
        )])
        .await
        .expect("second add should succeed");

    let results = store
        .search(&[0.0, 1.0], 1, None)
        .await
        .expect("search should succeed");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document.id, "doc1");
    assert_eq!(results[0].document.content, "new");
}

async fn contract_metadata_preserved(store: &dyn VectorStore) {
    store
        .add(vec![build_doc(
            "doc1",
            "with-meta",
            vec![1.0, 0.0],
            json!({"page": 5, "source": "test"}),
        )])
        .await
        .expect("add should succeed");

    let results = store
        .search(&[1.0, 0.0], 1, None)
        .await
        .expect("search should succeed");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document.metadata.get("page"), Some(&json!(5)));
    assert_eq!(
        results[0].document.metadata.get("source"),
        Some(&json!("test"))
    );
}

#[tokio::test]
async fn contract_chroma_add_single_vector() {
    if !chroma_contract_enabled() {
        return;
    }
    let store = make_chroma_store("add_single").await;
    contract_add_single_vector(store.as_ref()).await;
}

#[tokio::test]
async fn contract_chroma_search_returns_top_k() {
    if !chroma_contract_enabled() {
        return;
    }
    let store = make_chroma_store("top_k").await;
    contract_search_returns_top_k(store.as_ref()).await;
}

#[tokio::test]
async fn contract_chroma_delete_removes_vector() {
    if !chroma_contract_enabled() {
        return;
    }
    let store = make_chroma_store("delete").await;
    contract_delete_removes_vector(store.as_ref()).await;
}

#[tokio::test]
async fn contract_chroma_duplicate_id_overwrites() {
    if !chroma_contract_enabled() {
        return;
    }
    let store = make_chroma_store("duplicate").await;
    contract_duplicate_id_overwrites(store.as_ref()).await;
}

#[tokio::test]
async fn contract_chroma_metadata_preserved() {
    if !chroma_contract_enabled() {
        return;
    }
    let store = make_chroma_store("metadata").await;
    contract_metadata_preserved(store.as_ref()).await;
}

#[tokio::test]
async fn contract_inmemory_add_single_vector() {
    let store = make_in_memory_store();
    contract_add_single_vector(store.as_ref()).await;
}

#[tokio::test]
async fn contract_inmemory_search_returns_top_k() {
    let store = make_in_memory_store();
    contract_search_returns_top_k(store.as_ref()).await;
}

#[tokio::test]
async fn contract_inmemory_delete_removes_vector() {
    let store = make_in_memory_store();
    contract_delete_removes_vector(store.as_ref()).await;
}

#[tokio::test]
async fn contract_inmemory_duplicate_id_overwrites() {
    let store = make_in_memory_store();
    contract_duplicate_id_overwrites(store.as_ref()).await;
}

#[tokio::test]
async fn contract_inmemory_metadata_preserved() {
    let store = make_in_memory_store();
    contract_metadata_preserved(store.as_ref()).await;
}
