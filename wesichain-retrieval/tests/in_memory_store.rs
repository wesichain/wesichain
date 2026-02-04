use std::collections::HashMap;

use wesichain_core::{Document, MetadataFilter, Value, VectorStore};
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

#[tokio::test]
async fn in_memory_store_filters_metadata_eq() {
    let store = InMemoryVectorStore::new();
    let mut alpha_metadata = HashMap::new();
    alpha_metadata.insert("tag".to_string(), Value::String("alpha".to_string()));
    let mut beta_metadata = HashMap::new();
    beta_metadata.insert("tag".to_string(), Value::String("beta".to_string()));
    let docs = vec![
        Document {
            id: "a".to_string(),
            content: "a".to_string(),
            metadata: alpha_metadata,
            embedding: Some(vec![1.0, 0.0, 0.0]),
        },
        Document {
            id: "b".to_string(),
            content: "b".to_string(),
            metadata: beta_metadata,
            embedding: Some(vec![0.9, 0.1, 0.0]),
        },
    ];
    store.add(docs).await.unwrap();

    let filter = MetadataFilter::Eq("tag".to_string(), Value::String("alpha".to_string()));
    let results = store
        .search(&[1.0, 0.0, 0.0], 5, Some(&filter))
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document.id, "a");
}

#[tokio::test]
async fn in_memory_store_filters_metadata_in() {
    let store = InMemoryVectorStore::new();
    let mut a_metadata = HashMap::new();
    a_metadata.insert("group".to_string(), Value::String("a".to_string()));
    let mut c_metadata = HashMap::new();
    c_metadata.insert("group".to_string(), Value::String("c".to_string()));
    let docs = vec![
        Document {
            id: "a".to_string(),
            content: "a".to_string(),
            metadata: a_metadata,
            embedding: Some(vec![1.0, 0.0, 0.0]),
        },
        Document {
            id: "c".to_string(),
            content: "c".to_string(),
            metadata: c_metadata,
            embedding: Some(vec![0.9, 0.1, 0.0]),
        },
    ];
    store.add(docs).await.unwrap();

    let filter = MetadataFilter::In(
        "group".to_string(),
        vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
        ],
    );
    let results = store
        .search(&[1.0, 0.0, 0.0], 5, Some(&filter))
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document.id, "a");
}

#[tokio::test]
async fn in_memory_store_filters_metadata_range() {
    let store = InMemoryVectorStore::new();
    let mut low_metadata = HashMap::new();
    low_metadata.insert("score".to_string(), Value::Number(2.into()));
    let mut high_metadata = HashMap::new();
    high_metadata.insert("score".to_string(), Value::Number(12.into()));
    let docs = vec![
        Document {
            id: "low".to_string(),
            content: "low".to_string(),
            metadata: low_metadata,
            embedding: Some(vec![1.0, 0.0, 0.0]),
        },
        Document {
            id: "high".to_string(),
            content: "high".to_string(),
            metadata: high_metadata,
            embedding: Some(vec![0.9, 0.1, 0.0]),
        },
    ];
    store.add(docs).await.unwrap();

    let filter = MetadataFilter::Range {
        key: "score".to_string(),
        min: Some(Value::Number(1.into())),
        max: Some(Value::Number(10.into())),
    };
    let results = store
        .search(&[1.0, 0.0, 0.0], 5, Some(&filter))
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document.id, "low");
}

#[tokio::test]
async fn in_memory_store_filters_metadata_range_rejects_non_numeric_metadata() {
    let store = InMemoryVectorStore::new();
    let mut string_metadata = HashMap::new();
    string_metadata.insert("score".to_string(), Value::String("high".to_string()));
    let docs = vec![Document {
        id: "string".to_string(),
        content: "string".to_string(),
        metadata: string_metadata,
        embedding: Some(vec![1.0, 0.0, 0.0]),
    }];
    store.add(docs).await.unwrap();

    let filter = MetadataFilter::Range {
        key: "score".to_string(),
        min: Some(Value::String("a".to_string())),
        max: Some(Value::String("z".to_string())),
    };
    let results = store
        .search(&[1.0, 0.0, 0.0], 5, Some(&filter))
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn in_memory_store_filters_metadata_range_rejects_non_numeric_bounds() {
    let store = InMemoryVectorStore::new();
    let mut numeric_metadata = HashMap::new();
    numeric_metadata.insert("score".to_string(), Value::Number(7.into()));
    let docs = vec![Document {
        id: "numeric".to_string(),
        content: "numeric".to_string(),
        metadata: numeric_metadata,
        embedding: Some(vec![1.0, 0.0, 0.0]),
    }];
    store.add(docs).await.unwrap();

    let filter = MetadataFilter::Range {
        key: "score".to_string(),
        min: Some(Value::String("low".to_string())),
        max: None,
    };
    let results = store
        .search(&[1.0, 0.0, 0.0], 5, Some(&filter))
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn in_memory_store_filters_metadata_all() {
    let store = InMemoryVectorStore::new();
    let mut match_metadata = HashMap::new();
    match_metadata.insert("tag".to_string(), Value::String("alpha".to_string()));
    match_metadata.insert("score".to_string(), Value::Number(3.into()));
    let mut mismatch_metadata = HashMap::new();
    mismatch_metadata.insert("tag".to_string(), Value::String("alpha".to_string()));
    mismatch_metadata.insert("score".to_string(), Value::Number(20.into()));
    let docs = vec![
        Document {
            id: "match".to_string(),
            content: "match".to_string(),
            metadata: match_metadata,
            embedding: Some(vec![1.0, 0.0, 0.0]),
        },
        Document {
            id: "mismatch".to_string(),
            content: "mismatch".to_string(),
            metadata: mismatch_metadata,
            embedding: Some(vec![0.9, 0.1, 0.0]),
        },
    ];
    store.add(docs).await.unwrap();

    let filter = MetadataFilter::All(vec![
        MetadataFilter::Eq("tag".to_string(), Value::String("alpha".to_string())),
        MetadataFilter::Range {
            key: "score".to_string(),
            min: Some(Value::Number(1.into())),
            max: Some(Value::Number(10.into())),
        },
    ]);
    let results = store
        .search(&[1.0, 0.0, 0.0], 5, Some(&filter))
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document.id, "match");
}

#[tokio::test]
async fn in_memory_store_filters_metadata_any() {
    let store = InMemoryVectorStore::new();
    let mut alpha_metadata = HashMap::new();
    alpha_metadata.insert("tag".to_string(), Value::String("alpha".to_string()));
    let mut score_metadata = HashMap::new();
    score_metadata.insert("score".to_string(), Value::Number(7.into()));
    let mut other_metadata = HashMap::new();
    other_metadata.insert("tag".to_string(), Value::String("beta".to_string()));
    let docs = vec![
        Document {
            id: "alpha".to_string(),
            content: "alpha".to_string(),
            metadata: alpha_metadata,
            embedding: Some(vec![1.0, 0.0, 0.0]),
        },
        Document {
            id: "score".to_string(),
            content: "score".to_string(),
            metadata: score_metadata,
            embedding: Some(vec![0.9, 0.1, 0.0]),
        },
        Document {
            id: "other".to_string(),
            content: "other".to_string(),
            metadata: other_metadata,
            embedding: Some(vec![0.8, 0.2, 0.0]),
        },
    ];
    store.add(docs).await.unwrap();

    let filter = MetadataFilter::Any(vec![
        MetadataFilter::Eq("tag".to_string(), Value::String("alpha".to_string())),
        MetadataFilter::Range {
            key: "score".to_string(),
            min: Some(Value::Number(5.into())),
            max: Some(Value::Number(10.into())),
        },
    ]);
    let results = store
        .search(&[1.0, 0.0, 0.0], 5, Some(&filter))
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
    let ids: Vec<&str> = results
        .iter()
        .map(|result| result.document.id.as_str())
        .collect();
    assert!(ids.contains(&"alpha"));
    assert!(ids.contains(&"score"));
}

#[tokio::test]
async fn in_memory_store_strips_embeddings_from_results() {
    let store = InMemoryVectorStore::new();
    let docs = vec![Document {
        id: "a".to_string(),
        content: "a".to_string(),
        metadata: HashMap::new(),
        embedding: Some(vec![1.0, 0.0, 0.0]),
    }];
    store.add(docs).await.unwrap();

    let results = store.search(&[1.0, 0.0, 0.0], 1, None).await.unwrap();
    assert!(results[0].document.embedding.is_none());
}

#[tokio::test]
async fn in_memory_store_delete_is_idempotent() {
    let store = InMemoryVectorStore::new();
    let docs = vec![Document {
        id: "a".to_string(),
        content: "a".to_string(),
        metadata: HashMap::new(),
        embedding: Some(vec![1.0, 0.0, 0.0]),
    }];
    store.add(docs).await.unwrap();

    store.delete(&["a".to_string()]).await.unwrap();
    store.delete(&["a".to_string()]).await.unwrap();

    let results = store.search(&[1.0, 0.0, 0.0], 5, None).await.unwrap();
    assert!(results.is_empty());
}
