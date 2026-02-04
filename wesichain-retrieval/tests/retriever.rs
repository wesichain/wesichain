use std::collections::HashMap;
use wesichain_core::{Document, Embedding, VectorStore};
use wesichain_retrieval::{HashEmbedder, InMemoryVectorStore, Retriever};

#[tokio::test]
async fn retriever_returns_results() {
    let embedder = HashEmbedder::new(4);
    let query_embedder = embedder.clone();
    let store = InMemoryVectorStore::new();

    let docs = vec![Document {
        id: "doc".to_string(),
        content: "hello".to_string(),
        metadata: HashMap::new(),
        embedding: Some(query_embedder.embed("hello").await.unwrap()),
    }];
    store.add(docs).await.unwrap();

    let retriever = Retriever::new(embedder, store);
    let results = retriever.retrieve("hello", 1, None).await.unwrap();
    assert_eq!(results[0].document.id, "doc");
}
