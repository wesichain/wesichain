use std::collections::HashMap;

use wesichain_chroma::ChromaVectorStore;
use wesichain_core::Document;
use wesichain_rag::{RagQueryRequest, WesichainRag};
use wesichain_retrieval::HashEmbedder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = ChromaVectorStore::new("http://127.0.0.1:8000", "demo_docs").await?;

    let rag = WesichainRag::builder()
        .with_embedder(HashEmbedder::new(384))
        .with_vector_store(store)
        .build()?;

    rag.add_documents(vec![
        Document {
            id: "doc-1".to_string(),
            content:
                "Wesichain is a Rust-native LLM framework focused on graph and agent workflows."
                    .to_string(),
            metadata: HashMap::new(),
            embedding: None,
        },
        Document {
            id: "doc-2".to_string(),
            content: "Chroma stores vectors and metadata for similarity retrieval.".to_string(),
            metadata: HashMap::new(),
            embedding: None,
        },
    ])
    .await?;

    let response = rag
        .query(RagQueryRequest {
            query: "What is Wesichain focused on?".to_string(),
            thread_id: None,
        })
        .await?;

    println!("Answer: {}", response.answer);
    println!("Thread ID: {}", response.thread_id);
    Ok(())
}
