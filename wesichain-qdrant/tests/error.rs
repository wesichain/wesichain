use std::error::Error;

use wesichain_core::{Document, StoreError, VectorStore};
use wesichain_qdrant::{QdrantStoreError, QdrantVectorStore};

fn build_store(base_url: &str) -> QdrantVectorStore {
    QdrantVectorStore::builder()
        .base_url(base_url)
        .collection("docs")
        .build()
        .expect("store should build")
}

#[tokio::test]
async fn invalid_document_id_maps_to_store_invalid_id() {
    let store = build_store("http://127.0.0.1:6333");
    let docs = vec![Document {
        id: "   ".to_string(),
        content: "hello".to_string(),
        metadata: Default::default(),
        embedding: Some(vec![0.1, 0.2]),
    }];

    let err = store
        .add(docs)
        .await
        .expect_err("blank id should map to invalid id error");

    assert!(matches!(err, StoreError::InvalidId(id) if id == "   "));
}

#[tokio::test]
async fn qdrant_request_error_maps_to_internal_with_source_preserved() {
    let store = build_store("http://127.0.0.1:0");

    let err = store
        .search(&[0.1, 0.2], 1, None)
        .await
        .expect_err("request error should map to internal error");

    match err {
        StoreError::Internal(inner) => {
            let qdrant_err = inner
                .downcast_ref::<QdrantStoreError>()
                .expect("internal error should preserve qdrant source type");
            assert!(matches!(qdrant_err, QdrantStoreError::Request(_)));
            assert!(qdrant_err.source().is_some(), "qdrant source should be set");
        }
        other => panic!("expected StoreError::Internal, got: {other:?}"),
    }
}
