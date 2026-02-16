use wesichain_qdrant::{QdrantStoreError, QdrantVectorStore};

#[test]
fn builder_allows_optional_api_key_and_requires_collection() {
    let store = QdrantVectorStore::builder()
        .base_url("http://localhost:6333")
        .collection("docs")
        .build()
        .unwrap();
    assert_eq!(store.api_key(), None);

    let missing_collection = QdrantVectorStore::builder()
        .base_url("http://localhost:6333")
        .build()
        .unwrap_err();
    assert!(matches!(
        missing_collection,
        QdrantStoreError::MissingCollection
    ));
}

#[test]
fn builder_treats_whitespace_api_key_as_missing() {
    let store = QdrantVectorStore::builder()
        .base_url("https://example.cloud.qdrant.io")
        .collection("docs")
        .api_key("  \t\n  ")
        .build()
        .unwrap();

    assert_eq!(store.api_key(), None);
}

#[test]
fn builder_requires_base_url() {
    let missing_base_url = QdrantVectorStore::builder()
        .collection("docs")
        .build()
        .unwrap_err();

    assert!(matches!(missing_base_url, QdrantStoreError::MissingBaseUrl));
}

#[test]
fn builder_rejects_empty_or_whitespace_base_url() {
    let empty_base_url = QdrantVectorStore::builder()
        .base_url("")
        .collection("docs")
        .build()
        .unwrap_err();
    assert!(matches!(empty_base_url, QdrantStoreError::EmptyBaseUrl));

    let whitespace_base_url = QdrantVectorStore::builder()
        .base_url("   \t\n")
        .collection("docs")
        .build()
        .unwrap_err();
    assert!(matches!(
        whitespace_base_url,
        QdrantStoreError::EmptyBaseUrl
    ));
}

#[test]
fn builder_rejects_empty_or_whitespace_collection() {
    let empty_collection = QdrantVectorStore::builder()
        .base_url("http://localhost:6333")
        .collection("")
        .build()
        .unwrap_err();
    assert!(matches!(
        empty_collection,
        QdrantStoreError::EmptyCollection
    ));

    let whitespace_collection = QdrantVectorStore::builder()
        .base_url("http://localhost:6333")
        .collection(" \t\n")
        .build()
        .unwrap_err();
    assert!(matches!(
        whitespace_collection,
        QdrantStoreError::EmptyCollection
    ));
}

#[test]
fn builder_accepts_non_empty_values() {
    let store = QdrantVectorStore::builder()
        .base_url("http://localhost:6333")
        .collection("docs")
        .build()
        .unwrap();

    assert_eq!(store.base_url(), "http://localhost:6333");
    assert_eq!(store.collection(), "docs");
}
