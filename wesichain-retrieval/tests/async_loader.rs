use std::fs;

use tempfile::tempdir;
use wesichain_retrieval::{load_file_async, load_files_async, IngestionError, TextLoader};

#[tokio::test]
async fn async_loader_reads_txt_document() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("demo.txt");
    fs::write(&path, "Hello async ingestion!").expect("write temp file");

    let documents = load_file_async(path.clone())
        .await
        .expect("load txt asynchronously");

    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].id, path.to_string_lossy());
    assert_eq!(documents[0].content, "Hello async ingestion!");
}

#[tokio::test]
async fn async_loader_returns_unsupported_extension_error() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("demo.xyz");
    fs::write(&path, "unsupported content").expect("write temp file");

    let error = load_file_async(path.clone())
        .await
        .expect_err("unsupported extension should error");

    assert!(matches!(
        error,
        IngestionError::UnsupportedExtension {
            path: error_path,
            extension,
        } if error_path == path && extension == "xyz"
    ));
}

#[tokio::test]
async fn async_loader_is_additive_and_does_not_break_sync_text_loader() {
    let dir = tempdir().expect("temp dir");
    let async_path = dir.path().join("async.txt");
    let sync_path = dir.path().join("sync.txt");
    fs::write(&async_path, "async content").expect("write async temp file");
    fs::write(&sync_path, "sync content").expect("write sync temp file");

    let async_docs = load_files_async(vec![async_path.clone()])
        .await
        .expect("load async files");
    assert_eq!(async_docs.len(), 1);

    let sync_docs = TextLoader::new(sync_path.clone())
        .load()
        .expect("sync text loader should still work");
    assert_eq!(sync_docs.len(), 1);
    assert_eq!(sync_docs[0].content, "sync content");
    assert_eq!(sync_docs[0].id, sync_path.to_string_lossy());
}

#[tokio::test]
#[cfg(feature = "pdf")]
async fn async_loader_routes_pdf_extension_when_feature_enabled() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("missing.pdf");

    let error = load_file_async(path.clone())
        .await
        .expect_err("missing pdf should return read error");

    assert!(!matches!(
        error,
        IngestionError::UnsupportedExtension { .. }
    ));
    assert!(matches!(
        &error,
        IngestionError::Read {
            path: error_path,
            ..
        } | IngestionError::Parse {
            path: error_path,
            ..
        } if error_path == &path
    ));
}
