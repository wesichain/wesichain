use std::{collections::HashMap, sync::Arc};

use wesichain_core::Document;
use wesichain_retrieval::{HashEmbedder, InMemoryVectorStore, Indexer, Retriever, TextSplitter};

#[tokio::main]
async fn main() {
    let embedder = Arc::new(HashEmbedder::new(8));
    let store = Arc::new(InMemoryVectorStore::new());
    let indexer = Indexer::new(embedder.clone(), store.clone());

    let content = "Rust is fast and memory efficient.";
    let chunks = TextSplitter::split(content, 16, 4);
    let docs: Vec<Document> = chunks
        .into_iter()
        .enumerate()
        .map(|(idx, chunk)| Document {
            id: format!("doc-{idx}"),
            content: chunk,
            metadata: HashMap::new(),
            embedding: None,
        })
        .collect();

    indexer.index(docs).await.unwrap();

    let retriever = Retriever::new(embedder.clone(), store.clone());
    let results = retriever.retrieve("memory", 3, None).await.unwrap();
    println!("Retrieved {} docs", results.len());
    for result in results {
        println!(
            "score={:.3} content={}",
            result.score, result.document.content
        );
    }
}
