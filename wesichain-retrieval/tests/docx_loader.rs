use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use wesichain_retrieval::{load_file_async, IngestionError};
use zip::write::FileOptions;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn create_minimal_docx(path: &PathBuf, document_xml: &str) {
    let file = std::fs::File::create(path).expect("fixture docx file should be creatable");
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::default();

    zip.start_file("[Content_Types].xml", options)
        .expect("content types entry should start");
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )
    .expect("content types entry should write");

    zip.add_directory("_rels/", options)
        .expect("rels dir should be added");
    zip.start_file("_rels/.rels", options)
        .expect("rels entry should start");
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    )
    .expect("rels entry should write");

    zip.add_directory("word/", options)
        .expect("word dir should be added");
    zip.start_file("word/document.xml", options)
        .expect("document entry should start");
    zip.write_all(document_xml.as_bytes())
        .expect("document entry should write");

    zip.finish().expect("docx archive should finalize");
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

#[tokio::test]
async fn docx_loader_preserves_run_boundary_spaces() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("docx-run-spacing-{nonce}.docx"));

    create_minimal_docx(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t xml:space="preserve">Hello </w:t></w:r>
      <w:r><w:t>world</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#,
    );

    let documents = load_file_async(path.clone())
        .await
        .expect("generated docx should load");

    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].content, "Hello world");

    let _ = std::fs::remove_file(path);
}
