use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use wesichain_core::{
    Document, Embedding, HasMetadataFilter, HasQuery, HasRetrievedDocs, MetadataFilter, Runnable,
    VectorStore,
};
use wesichain_graph::{GraphState, RetrieverNode, StateSchema, StateUpdate};
use wesichain_retrieval::{HashEmbedder, InMemoryVectorStore};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    query: String,
    docs: Vec<Document>,
}

impl StateSchema for DemoState {}

impl HasQuery for DemoState {
    fn query(&self) -> &str {
        &self.query
    }
}

impl HasRetrievedDocs for DemoState {
    fn set_retrieved_docs(&mut self, docs: Vec<Document>) {
        self.docs = docs;
    }
}

impl HasMetadataFilter for DemoState {
    fn metadata_filter(&self) -> Option<MetadataFilter> {
        None
    }
}

#[tokio::test]
async fn retriever_node_updates_state() {
    let embedder = Arc::new(HashEmbedder::new(4));
    let store = Arc::new(InMemoryVectorStore::new());

    let docs = vec![Document {
        id: "doc".to_string(),
        content: "hello".to_string(),
        metadata: HashMap::new(),
        embedding: Some(embedder.embed("hello").await.unwrap()),
    }];
    store.add(docs).await.unwrap();

    let node = RetrieverNode::new(embedder, store, 1, None);
    let state = GraphState::new(DemoState {
        query: "hello".to_string(),
        docs: Vec::new(),
    });

    let update: StateUpdate<DemoState> = node.invoke(state).await.unwrap();
    assert_eq!(update.data.docs.len(), 1);
}
