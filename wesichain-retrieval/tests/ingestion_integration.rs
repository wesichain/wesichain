use std::fs;
use std::path::PathBuf;

use tempfile::tempdir;
use wesichain_retrieval::{load_and_split_recursive, RecursiveCharacterTextSplitter};

fn docx_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[tokio::test]
async fn ingestion_integration_load_and_split_recursive_ingests_txt_and_docx_with_expected_metadata() {
    let dir = tempdir().expect("temp dir");
    let txt_path = dir.path().join("notes.txt");
    fs::write(
        &txt_path,
        "alpha beta gamma delta epsilon zeta eta theta iota kappa",
    )
    .expect("write temp text fixture");
    let docx_path = docx_fixture_path("paragraphs_table.docx");

    let splitter = RecursiveCharacterTextSplitter::builder()
        .chunk_size(20)
        .chunk_overlap(5)
        .build()
        .expect("splitter should build");

    let chunks = load_and_split_recursive(vec![txt_path.clone(), docx_path.clone()], &splitter)
        .await
        .expect("ingestion and split should succeed");

    assert!(chunks.len() > 2);

    let mut saw_txt = false;
    let mut saw_docx = false;

    for chunk in &chunks {
        let source = chunk
            .metadata
            .get("source")
            .and_then(|value| value.as_str())
            .expect("chunk source metadata");
        let chunk_index = chunk
            .metadata
            .get("chunk_index")
            .and_then(|value| value.as_u64())
            .expect("chunk index metadata");

        assert!(chunk.id.ends_with(&format!(":{chunk_index}")));
        assert!(!chunk.content.is_empty());

        if source.ends_with("notes.txt") {
            saw_txt = true;
        }
        if source.ends_with("paragraphs_table.docx") {
            saw_docx = true;
        }
    }

    assert!(saw_txt, "expected at least one txt chunk");
    assert!(saw_docx, "expected at least one docx chunk");
}
