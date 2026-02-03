use wesichain_core::Embedding;
use wesichain_retrieval::HashEmbedder;

#[tokio::test]
async fn hash_embedder_is_deterministic() {
    let embedder = HashEmbedder::new(4);
    let first = embedder.embed("hello").await.unwrap();
    let second = embedder.embed("hello").await.unwrap();
    assert_eq!(first, second);
}

#[tokio::test]
async fn hash_embedder_batch_matches_single() {
    let embedder = HashEmbedder::new(4);
    let batch = embedder
        .embed_batch(&["hello".to_string()])
        .await
        .unwrap();
    let single = embedder.embed("hello").await.unwrap();
    assert_eq!(batch[0], single);
}
