use std::fs;

use serde_json::Value;
use tempfile::tempdir;
use wesichain_retrieval::{PdfLoader, TextLoader};

#[test]
fn text_loader_returns_document_with_metadata() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("demo.txt");
    fs::write(&path, "Hello, Wesichain!").expect("write temp file");

    let loader = TextLoader::new(path.clone());
    let documents = loader.load().expect("load temp file");

    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].content, "Hello, Wesichain!");
    assert_eq!(documents[0].id, path.to_string_lossy());
    assert_eq!(documents[0].embedding, None);
    assert_eq!(
        documents[0].metadata.get("source"),
        Some(&Value::String(path.to_string_lossy().into_owned()))
    );
}

#[cfg(not(feature = "pdf"))]
#[test]
fn pdf_loader_returns_error_when_feature_disabled() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("demo.pdf");
    fs::write(&path, "not a real pdf").expect("write temp file");

    let loader = PdfLoader::new(path);
    let err = loader.load().expect_err("pdf load should error");

    assert_eq!(err.kind(), std::io::ErrorKind::Other);
}
