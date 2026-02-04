use std::collections::HashMap;

use wesichain_core::{Document, Value};

#[test]
fn document_roundtrip() {
    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), Value::String("unit".to_string()));

    let doc = Document {
        id: "doc-1".to_string(),
        content: "hello".to_string(),
        metadata,
        embedding: Some(vec![1.0, 0.0, 0.0]),
    };

    let json = serde_json::to_string(&doc).unwrap();
    let parsed: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(doc, parsed);
}
