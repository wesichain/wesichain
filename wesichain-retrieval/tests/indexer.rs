use std::collections::HashMap;

use wesichain_core::{Document, Embedding, VectorStore};
use wesichain_retrieval::{HashEmbedder, InMemoryVectorStore, Indexer, RetrievalError};

#[tokio::test]
async fn indexer_rejects_empty_id() {
    let embedder = HashEmbedder::new(8);
    let store = InMemoryVectorStore::new();
    let indexer = Indexer::new(embedder, store);

    let doc = Document {
        id: "   ".to_string(),
        content: "hello".to_string(),
        metadata: HashMap::new(),
        embedding: None,
    };

    let error = indexer.index(vec![doc]).await.unwrap_err();

    assert!(matches!(error, RetrievalError::InvalidId(id) if id.trim().is_empty()));
}

#[tokio::test]
async fn indexer_embeds_and_adds_documents() {
    let embedder = HashEmbedder::new(8);
    let query_embedder = embedder.clone();
    let store = InMemoryVectorStore::new();
    let indexer = Indexer::new(embedder, store.clone());

    let docs = vec![
        Document {
            id: "doc-1".to_string(),
            content: "first document".to_string(),
            metadata: HashMap::new(),
            embedding: None,
        },
        Document {
            id: "doc-2".to_string(),
            content: "second document".to_string(),
            metadata: HashMap::new(),
            embedding: None,
        },
    ];

    indexer.index(docs).await.unwrap();

    let query_embedding = query_embedder.embed("first document").await.unwrap();
    let results = store.search(&query_embedding, 1, None).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document.id, "doc-1");
    assert_eq!(results[0].document.content, "first document");
}
