use wesichain_weaviate::{WeaviateStoreError, WeaviateVectorStore};

#[test]
fn builder_allows_optional_api_key_and_requires_class() {
    let store = WeaviateVectorStore::builder()
        .base_url("http://localhost:8080")
        .class_name("Doc")
        .build()
        .unwrap();
    assert_eq!(store.api_key(), None);

    let missing_class = WeaviateVectorStore::builder()
        .base_url("http://localhost:8080")
        .build()
        .unwrap_err();
    assert!(matches!(
        missing_class,
        WeaviateStoreError::MissingClassName
    ));
}

#[test]
fn builder_requires_base_url() {
    let missing_base_url = WeaviateVectorStore::builder()
        .class_name("Doc")
        .build()
        .unwrap_err();

    assert!(matches!(
        missing_base_url,
        WeaviateStoreError::MissingBaseUrl
    ));
}

#[test]
fn builder_rejects_empty_or_whitespace_values() {
    let empty_base_url = WeaviateVectorStore::builder()
        .base_url("")
        .class_name("Doc")
        .build()
        .unwrap_err();
    assert!(matches!(empty_base_url, WeaviateStoreError::EmptyBaseUrl));

    let whitespace_base_url = WeaviateVectorStore::builder()
        .base_url("   \t\n")
        .class_name("Doc")
        .build()
        .unwrap_err();
    assert!(matches!(
        whitespace_base_url,
        WeaviateStoreError::EmptyBaseUrl
    ));

    let empty_class_name = WeaviateVectorStore::builder()
        .base_url("http://localhost:8080")
        .class_name("")
        .build()
        .unwrap_err();
    assert!(matches!(
        empty_class_name,
        WeaviateStoreError::EmptyClassName
    ));

    let whitespace_class_name = WeaviateVectorStore::builder()
        .base_url("http://localhost:8080")
        .class_name(" \t\n")
        .build()
        .unwrap_err();
    assert!(matches!(
        whitespace_class_name,
        WeaviateStoreError::EmptyClassName
    ));
}

#[test]
fn debug_redacts_api_key() {
    let store = WeaviateVectorStore::builder()
        .base_url("https://foo.cloud.weaviate.io")
        .class_name("Doc")
        .api_key("secret-key")
        .build()
        .unwrap();

    let formatted = format!("{store:?}");
    assert!(formatted.contains("<redacted>"));
    assert!(!formatted.contains("secret-key"));
}

#[test]
fn builder_trims_base_url_and_class_name_on_build() {
    let store = WeaviateVectorStore::builder()
        .base_url("  http://localhost:8080  ")
        .class_name("  Doc  ")
        .build()
        .unwrap();

    assert_eq!(store.base_url(), "http://localhost:8080");
    assert_eq!(store.class_name(), "Doc");
}

#[test]
fn builder_treats_whitespace_api_key_as_none() {
    let store = WeaviateVectorStore::builder()
        .base_url("http://localhost:8080")
        .class_name("Doc")
        .api_key(" \t\n ")
        .build()
        .unwrap();

    assert_eq!(store.api_key(), None);
}

#[test]
fn builder_trims_api_key_before_storing() {
    let store = WeaviateVectorStore::builder()
        .base_url("http://localhost:8080")
        .class_name("Doc")
        .api_key("  secret-key  ")
        .build()
        .unwrap();

    assert_eq!(store.api_key(), Some("secret-key"));
}

#[test]
fn builder_rejects_unsafe_class_names() {
    let invalid_chars = WeaviateVectorStore::builder()
        .base_url("http://localhost:8080")
        .class_name("Doc-Name")
        .build()
        .unwrap_err();
    assert!(matches!(
        invalid_chars,
        WeaviateStoreError::InvalidClassName { .. }
    ));

    let starts_with_digit = WeaviateVectorStore::builder()
        .base_url("http://localhost:8080")
        .class_name("1Doc")
        .build()
        .unwrap_err();
    assert!(matches!(
        starts_with_digit,
        WeaviateStoreError::InvalidClassName { .. }
    ));
}
