use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use wesichain_core::{
    Document, Embedding, HasMetadataFilter, HasQuery, HasRetrievedDocs, MetadataFilter, VectorStore,
};
use wesichain_graph::{GraphBuilder, GraphState, RetrieverNode, StateSchema};
use wesichain_retrieval::{HashEmbedder, InMemoryVectorStore};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    query: String,
    docs: Vec<Document>,
}

impl StateSchema for DemoState {
    type Update = Self;
    fn apply(_: &Self, update: Self) -> Self {
        update
    }
}

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

#[tokio::main]
async fn main() {
    let embedder = Arc::new(HashEmbedder::new(8));
    let store = Arc::new(InMemoryVectorStore::new());

    let docs = vec![Document {
        id: "doc".to_string(),
        content: "Rust is fast.".to_string(),
        metadata: HashMap::new(),
        embedding: Some(embedder.embed("Rust is fast.").await.unwrap()),
    }];
    store.add(docs).await.unwrap();

    let node = RetrieverNode::new(embedder, store, 3, None);
    let graph = GraphBuilder::new()
        .add_node("retrieve", node)
        .set_entry("retrieve")
        .build();

    let state = GraphState::new(DemoState {
        query: "Rust".to_string(),
        docs: Vec::new(),
    });

    let start = Instant::now();
    let out = graph.invoke(state).await.unwrap();
    println!(
        "Retrieved {} docs in {:?}",
        out.data.docs.len(),
        start.elapsed()
    );
}
