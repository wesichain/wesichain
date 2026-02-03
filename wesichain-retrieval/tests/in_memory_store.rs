use std::collections::HashMap;

use wesichain_core::{Document, VectorStore};
use wesichain_retrieval::InMemoryVectorStore;

#[tokio::test]
async fn ranks_by_cosine_similarity() {
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
async fn dimension_mismatch_on_add() {
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
