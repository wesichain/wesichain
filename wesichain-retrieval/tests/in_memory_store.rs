use std::collections::HashMap;

use wesichain_core::{Document, VectorStore};
use wesichain_retrieval::InMemoryVectorStore;

#[tokio::test]
async fn in_memory_store_ranks_by_cosine_similarity() {
    let store = InMemoryVectorStore::new();
    let docs = vec![
        Document {
            id: "a".to_string(),
            content: "a".to_string(),
            metadata: HashMap::new(),
            embedding: Some(vec![1.0, 0.0, 0.0]),
        },
        Document {
            id: "b".to_string(),
            content: "b".to_string(),
            metadata: HashMap::new(),
            embedding: Some(vec![0.0, 1.0, 0.0]),
        },
    ];
    store.add(docs).await.unwrap();

    let results = store.search(&[1.0, 0.0, 0.0], 1, None).await.unwrap();
    assert_eq!(results[0].document.id, "a");
}

#[tokio::test]
async fn in_memory_store_dimension_mismatch_on_add() {
    let store = InMemoryVectorStore::new();
    let docs = vec![Document {
        id: "a".to_string(),
        content: "a".to_string(),
        metadata: HashMap::new(),
        embedding: Some(vec![1.0, 0.0]),
    }];
    store.add(docs).await.unwrap();

    let mismatch = vec![Document {
        id: "b".to_string(),
        content: "b".to_string(),
        metadata: HashMap::new(),
        embedding: Some(vec![1.0, 0.0, 0.0]),
    }];
    let err = store.add(mismatch).await.unwrap_err();
    assert!(format!("{err}").contains("dimension mismatch"));
}

#[tokio::test]
async fn in_memory_store_duplicate_ids_overwrite_existing_doc() {
    let store = InMemoryVectorStore::new();
    let docs = vec![Document {
        id: "a".to_string(),
        content: "first".to_string(),
        metadata: HashMap::new(),
        embedding: Some(vec![1.0, 0.0, 0.0]),
    }];
    store.add(docs).await.unwrap();

    let overwrite = vec![Document {
        id: "a".to_string(),
        content: "second".to_string(),
        metadata: HashMap::new(),
        embedding: Some(vec![1.0, 0.0, 0.0]),
    }];
    store.add(overwrite).await.unwrap();

    let results = store.search(&[1.0, 0.0, 0.0], 5, None).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document.content, "second");
}

#[tokio::test]
async fn in_memory_store_nan_scores_do_not_panic() {
    let store = InMemoryVectorStore::new();
    let docs = vec![
        Document {
            id: "a".to_string(),
            content: "a".to_string(),
            metadata: HashMap::new(),
            embedding: Some(vec![f32::NAN, 0.0, 0.0]),
        },
        Document {
            id: "b".to_string(),
            content: "b".to_string(),
            metadata: HashMap::new(),
            embedding: Some(vec![0.0, 1.0, 0.0]),
        },
    ];
    store.add(docs).await.unwrap();

    let results = store.search(&[1.0, 0.0, 0.0], 5, None).await.unwrap();
    assert_eq!(results.len(), 2);
}
