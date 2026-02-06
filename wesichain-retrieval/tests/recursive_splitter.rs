use std::collections::HashMap;

use serde_json::json;
use wesichain_core::Document;
use wesichain_retrieval::{RecursiveCharacterTextSplitter, SplitterConfigError};

#[test]
fn recursive_splitter_respects_separator_priority() {
    let splitter = RecursiveCharacterTextSplitter::builder()
        .chunk_size(8)
        .chunk_overlap(0)
        .build()
        .unwrap();

    let text = "aa aa\n\nbb bb\n\ncc cc";
    let chunks = splitter.split_text(text);

    assert_eq!(chunks, vec!["aa aa\n\n", "bb bb\n\n", "cc cc"]);
    assert_eq!(chunks.concat(), text);
}

#[test]
fn recursive_splitter_preserves_utf8_boundaries() {
    let splitter = RecursiveCharacterTextSplitter::builder()
        .chunk_size(3)
        .chunk_overlap(0)
        .build()
        .unwrap();

    let text = "a🙂b🙂c🙂";
    let chunks = splitter.split_text(text);
    let reconstructed = chunks.concat();

    assert_eq!(reconstructed, text);
    assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 3));
}

#[test]
fn recursive_splitter_applies_overlap_windows() {
    let splitter = RecursiveCharacterTextSplitter::builder()
        .chunk_size(4)
        .chunk_overlap(1)
        .build()
        .unwrap();

    let chunks = splitter.split_text("abcdefghij");

    assert_eq!(chunks, vec!["abcd", "defg", "ghij"]);
}

#[test]
fn recursive_splitter_rejects_zero_chunk_size() {
    let error = RecursiveCharacterTextSplitter::builder()
        .chunk_size(0)
        .build()
        .unwrap_err();

    assert!(matches!(
        error,
        SplitterConfigError::ChunkSizeMustBeGreaterThanZero
    ));
}

#[test]
fn recursive_splitter_clamps_overlap_to_allow_progress() {
    let splitter = RecursiveCharacterTextSplitter::builder()
        .chunk_size(3)
        .chunk_overlap(9)
        .build()
        .unwrap();

    let chunks = splitter.split_text("abcd");

    assert_eq!(chunks, vec!["abc", "bcd"]);
}

#[test]
fn recursive_splitter_split_documents_propagates_metadata() {
    let splitter = RecursiveCharacterTextSplitter::builder()
        .chunk_size(5)
        .chunk_overlap(0)
        .build()
        .unwrap();

    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), json!("unit-test.txt"));
    metadata.insert("category".to_string(), json!("test"));

    let input_doc = Document {
        id: "doc-1".to_string(),
        content: "one two three".to_string(),
        metadata,
        embedding: None,
    };

    let chunked_docs = splitter.split_documents(&[input_doc]);

    assert!(chunked_docs.len() > 1);
    for (index, doc) in chunked_docs.iter().enumerate() {
        assert_eq!(doc.metadata.get("source"), Some(&json!("unit-test.txt")));
        assert_eq!(doc.metadata.get("category"), Some(&json!("test")));
        assert_eq!(doc.metadata.get("chunk_index"), Some(&json!(index)));
        assert!(doc.content.chars().count() <= 5);
    }
}
