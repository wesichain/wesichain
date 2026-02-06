use std::collections::HashMap;

use serde_json::Value;
use wesichain_core::Document;

use crate::error::PineconeStoreError;

pub fn doc_to_metadata(doc: &Document, text_key: &str) -> HashMap<String, Value> {
    let mut metadata = doc.metadata.clone();
    metadata.insert(text_key.to_string(), Value::String(doc.content.clone()));
    metadata
}

pub fn match_to_document(
    id: &str,
    metadata: &Value,
    text_key: &str,
) -> Result<Document, PineconeStoreError> {
    let object = metadata.as_object().ok_or_else(|| {
        PineconeStoreError::Malformed("match metadata must be an object".to_string())
    })?;
    let text = object
        .get(text_key)
        .and_then(Value::as_str)
        .ok_or_else(|| PineconeStoreError::MissingTextKey {
            text_key: text_key.to_string(),
        })?
        .to_string();

    let mut out = HashMap::new();
    for (k, v) in object {
        if k != text_key {
            out.insert(k.clone(), v.clone());
        }
    }

    Ok(Document {
        id: id.to_string(),
        content: text,
        metadata: out,
        embedding: None,
    })
}
