use std::collections::HashMap;

use serde_json::json;
use wesichain_core::{Document, SearchResult, StoreError, Value, VectorStore};
use wesichain_qdrant::QdrantVectorStore;

fn guide_metadata() -> HashMap<String, Value> {
    HashMap::from([("source".to_string(), json!("guide"))])
}

pub fn sample_documents() -> Vec<Document> {
    vec![
        Document {
            id: "doc-1".to_string(),
            content:
                "Wesichain is a Rust-native LLM framework focused on graph and agent workflows."
                    .to_string(),
            metadata: guide_metadata(),
            embedding: Some(vec![0.99, 0.01, 0.0]),
        },
        Document {
            id: "doc-2".to_string(),
            content: "Qdrant stores vectors and metadata for similarity retrieval.".to_string(),
            metadata: guide_metadata(),
            embedding: Some(vec![0.70, 0.30, 0.0]),
        },
    ]
}

pub async fn run_core_flow(
    store: &QdrantVectorStore,
    docs: Vec<Document>,
) -> Result<Vec<SearchResult>, StoreError> {
    store.add(docs).await?;

    let results = store.search(&[0.98, 0.02, 0.0], 2, None).await?;

    let ids = results
        .iter()
        .map(|result| result.document.id.clone())
        .collect::<Vec<_>>();
    store.delete(&ids).await?;

    Ok(results)
}

#[cfg(not(test))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://127.0.0.1:6333".to_string());
    let collection =
        std::env::var("QDRANT_COLLECTION").unwrap_or_else(|_| "wesichain_docs".to_string());

    let builder = QdrantVectorStore::builder()
        .base_url(base_url)
        .collection(collection);
    let builder = match std::env::var("QDRANT_API_KEY") {
        Ok(api_key) if !api_key.trim().is_empty() => builder.api_key(api_key),
        _ => builder,
    };

    let store = builder.build()?;
    let results = run_core_flow(&store, sample_documents()).await?;

    for result in results {
        println!(
            "id={} score={:.3} content={}",
            result.document.id, result.score, result.document.content
        );
    }

    Ok(())
}
