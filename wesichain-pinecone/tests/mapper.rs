use std::collections::HashMap;

use serde_json::json;
use wesichain_core::Document;
use wesichain_pinecone::mapper::{doc_to_metadata, match_to_document};

#[test]
fn doc_to_metadata_preserves_fields_and_text_key() {
    let mut meta = HashMap::new();
    meta.insert("source".to_string(), json!("tweet"));
    let doc = Document {
        id: "d1".to_string(),
        content: "hello".to_string(),
        metadata: meta,
        embedding: None,
    };
    let out = doc_to_metadata(&doc, "text");
    assert_eq!(out.get("text"), Some(&json!("hello")));
    assert_eq!(out.get("source"), Some(&json!("tweet")));
}

#[test]
fn match_to_document_reads_text_key() {
    let metadata = json!({"text": "body", "source": "tweet"});
    let doc = match_to_document("id-1", &metadata, "text").unwrap();
    assert_eq!(doc.id, "id-1");
    assert_eq!(doc.content, "body");
    assert_eq!(doc.metadata.get("source"), Some(&json!("tweet")));
}
