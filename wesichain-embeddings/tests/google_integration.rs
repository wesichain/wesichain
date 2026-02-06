//! Integration tests for Google embeddings provider
//! Run with: cargo test -p wesichain-embeddings --features google -- --ignored

#![cfg(feature = "google")]

use wesichain_core::Embedding;
use wesichain_embeddings::GoogleEmbedding;

#[tokio::test]
#[ignore = "Requires GOOGLE_API_KEY environment variable"]
async fn test_google_embed_single_text() {
    let api_key = std::env::var("GOOGLE_API_KEY").expect("GOOGLE_API_KEY not set");
    let embedder = GoogleEmbedding::new(api_key, "text-embedding-004", 768);

    let vector = embedder.embed("hello world").await.expect("embed failed");
    assert_eq!(vector.len(), 768);
}

#[tokio::test]
#[ignore = "Requires GOOGLE_API_KEY environment variable"]
async fn test_google_embed_batch() {
    let api_key = std::env::var("GOOGLE_API_KEY").expect("GOOGLE_API_KEY not set");
    let embedder = GoogleEmbedding::new(api_key, "text-embedding-004", 768);

    let inputs = vec!["hello".to_string(), "world".to_string()];
    let vectors = embedder.embed_batch(&inputs).await.expect("batch failed");

    assert_eq!(vectors.len(), 2);
    assert_eq!(vectors[0].len(), 768);
    assert_eq!(vectors[1].len(), 768);
}
