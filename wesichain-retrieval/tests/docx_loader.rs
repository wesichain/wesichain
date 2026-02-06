use std::path::PathBuf;

use serde_json::Value;
use wesichain_retrieval::{load_file_async, IngestionError};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[tokio::test]
async fn docx_loader_flattens_paragraphs_and_table_cells() {
    let path = fixture_path("paragraphs_table.docx");

    let documents = load_file_async(path.clone())
        .await
        .expect("docx fixture should load");

    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].id, path.to_string_lossy());
    assert_eq!(
        documents[0].content,
        "Intro paragraph\n\nSecond paragraph\n\nR1C1 | R1C2\nR2C1 | R2C2"
    );
    assert_eq!(
        documents[0].metadata.get("source"),
        Some(&Value::String(path.to_string_lossy().into_owned()))
    );
}

#[tokio::test]
async fn docx_loader_returns_parse_error_for_malformed_docx() {
    let path = fixture_path("malformed.docx");

    let error = load_file_async(path.clone())
        .await
        .expect_err("malformed docx should produce parse error");

    assert!(matches!(error, IngestionError::Parse { path: err_path, .. } if err_path == path));
}
