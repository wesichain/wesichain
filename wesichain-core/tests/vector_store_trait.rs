use std::{error::Error, sync::Arc, sync::Mutex};

use async_trait::async_trait;
use wesichain_core::{
    delete_ref_dyn, delete_strs_dyn, Document, MetadataFilter, SearchResult, StoreError,
    VectorStore,
};

#[derive(Clone, Default)]
struct RecordingStore {
    deleted: Arc<Mutex<Vec<Vec<String>>>>,
}

impl RecordingStore {
    fn new() -> Self {
        Self {
            deleted: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl VectorStore for RecordingStore {
    async fn add(&self, _docs: Vec<Document>) -> Result<(), StoreError> {
        Ok(())
    }

    async fn search(
        &self,
        _query_embedding: &[f32],
        _top_k: usize,
        _filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, StoreError> {
        Ok(Vec::new())
    }

    async fn delete(&self, ids: &[String]) -> Result<(), StoreError> {
        self.deleted.lock().unwrap().push(ids.to_vec());
        Ok(())
    }
}

#[test]
fn vector_store_trait_object_safe() {
    let _: Option<Arc<dyn VectorStore>> = None;
}

#[tokio::test]
async fn vector_store_trait_delete_helpers_forward_ids_for_concrete_store() {
    let store = RecordingStore::new();

    store.delete_strs(&["a", "b"]).await.unwrap();

    let owned = vec!["c".to_string(), "d".to_string()];
    store.delete_ref(&owned).await.unwrap();

    let deleted = store.deleted.lock().unwrap().clone();
    assert_eq!(
        deleted,
        vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["c".to_string(), "d".to_string()],
        ]
    );
}

#[tokio::test]
async fn vector_store_trait_delete_helpers_forward_ids_for_dyn_store() {
    let store = RecordingStore::new();
    let deleted = store.deleted.clone();
    let store: Arc<dyn VectorStore> = Arc::new(store);

    delete_strs_dyn(store.as_ref(), &["x", "y"]).await.unwrap();

    let owned = vec!["z".to_string()];
    delete_ref_dyn(store.as_ref(), &owned).await.unwrap();

    let deleted = deleted.lock().unwrap().clone();
    assert_eq!(
        deleted,
        vec![
            vec!["x".to_string(), "y".to_string()],
            vec!["z".to_string()],
        ]
    );
}

#[test]
fn vector_store_trait_store_error_internal_preserves_source() {
    let source = std::io::Error::new(std::io::ErrorKind::Other, "disk");
    let err = StoreError::Internal(Box::new(source));

    assert_eq!(format!("{err}"), "Store error: disk");
    assert!(err.source().is_some());
}
