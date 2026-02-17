use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use wesichain_core::{Document, SearchResult, Value};

use crate::WeaviateStoreError;

pub const CONTENT_PAYLOAD_KEY: &str = "__wesichain_content";
pub const METADATA_PAYLOAD_KEY: &str = "__wesichain_metadata";

#[derive(Debug, Clone, Serialize)]
pub struct WeaviateObject {
    pub class: String,
    pub id: String,
    pub vector: Vec<f32>,
    pub properties: JsonMap<String, JsonValue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphQlRequest {
    pub query: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaCreateRequest {
    pub class: String,
    #[serde(rename = "vectorizer")]
    pub vectorizer: String,
    #[serde(rename = "properties")]
    pub properties: Vec<SchemaProperty>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaProperty {
    pub name: String,
    #[serde(rename = "dataType")]
    pub data_type: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct GraphQlResponse {
    #[serde(default)]
    pub data: Option<JsonValue>,
    #[serde(default)]
    pub errors: Vec<GraphQlError>,
}

#[derive(Debug, Deserialize)]
pub struct GraphQlError {
    pub message: String,
}

pub fn doc_to_object(
    mut doc: Document,
    class_name: &str,
) -> Result<WeaviateObject, WeaviateStoreError> {
    if doc.id.trim().is_empty() {
        return Err(WeaviateStoreError::InvalidDocumentId(doc.id));
    }

    let vector = doc
        .embedding
        .take()
        .ok_or_else(|| WeaviateStoreError::MissingEmbedding { id: doc.id.clone() })?;

    let mut properties = JsonMap::new();
    properties.insert(
        CONTENT_PAYLOAD_KEY.to_string(),
        JsonValue::String(doc.content),
    );
    let metadata_payload = serde_json::to_string(&doc.metadata).map_err(|err| {
        WeaviateStoreError::InvalidResponse {
            message: format!("failed to serialize metadata payload: {err}"),
        }
    })?;
    properties.insert(
        METADATA_PAYLOAD_KEY.to_string(),
        JsonValue::String(metadata_payload),
    );

    Ok(WeaviateObject {
        class: class_name.to_string(),
        id: doc.id,
        vector,
        properties,
    })
}

pub fn class_schema_request(class_name: &str) -> SchemaCreateRequest {
    SchemaCreateRequest {
        class: class_name.to_string(),
        vectorizer: "none".to_string(),
        properties: vec![
            SchemaProperty {
                name: CONTENT_PAYLOAD_KEY.to_string(),
                data_type: vec!["text".to_string()],
            },
            SchemaProperty {
                name: METADATA_PAYLOAD_KEY.to_string(),
                data_type: vec!["text".to_string()],
            },
        ],
    }
}

pub fn build_near_vector_query(
    class_name: &str,
    query_embedding: &[f32],
    top_k: usize,
    where_clause: Option<&str>,
) -> String {
    let embedding = query_embedding
        .iter()
        .map(|value| {
            let mut out = value.to_string();
            if !out.contains('.') && !out.contains('e') && !out.contains('E') {
                out.push_str(".0");
            }
            out
        })
        .collect::<Vec<String>>()
        .join(",");

    let where_clause = where_clause
        .map(|clause| format!(",where:{clause}"))
        .unwrap_or_default();

    format!(
        "{{Get{{{class_name}(nearVector:{{vector:[{embedding}]}},limit:{top_k}{where_clause}){{_additional{{id certainty}} {CONTENT_PAYLOAD_KEY} {METADATA_PAYLOAD_KEY}}}}}}}"
    )
}

pub fn graphql_hits_to_results(
    data: JsonValue,
    class_name: &str,
) -> Result<Vec<SearchResult>, WeaviateStoreError> {
    let hits = data
        .get("Get")
        .and_then(|value| value.get(class_name))
        .and_then(JsonValue::as_array)
        .ok_or_else(|| WeaviateStoreError::InvalidResponse {
            message: format!("missing data.Get.{class_name} array in GraphQL response"),
        })?;

    let mut results = Vec::with_capacity(hits.len());
    for hit in hits {
        let id = hit
            .get("_additional")
            .and_then(|additional| additional.get("id"))
            .and_then(JsonValue::as_str)
            .ok_or_else(|| WeaviateStoreError::InvalidResponse {
                message: "missing _additional.id in GraphQL hit".to_string(),
            })?
            .to_string();

        let score = hit
            .get("_additional")
            .and_then(|additional| additional.get("certainty"))
            .and_then(JsonValue::as_f64)
            .map(|value| value as f32)
            .unwrap_or(0.0);

        let content = hit
            .get(CONTENT_PAYLOAD_KEY)
            .and_then(JsonValue::as_str)
            .ok_or_else(|| WeaviateStoreError::MissingContentPayload {
                object_id: id.clone(),
            })?
            .to_string();

        let metadata_json = hit
            .get(METADATA_PAYLOAD_KEY)
            .and_then(JsonValue::as_str)
            .ok_or_else(|| WeaviateStoreError::InvalidMetadataPayload {
                object_id: id.clone(),
                message: "missing metadata payload".to_string(),
            })?;

        let metadata: HashMap<String, Value> =
            serde_json::from_str(metadata_json).map_err(|err| {
                WeaviateStoreError::InvalidMetadataPayload {
                    object_id: id.clone(),
                    message: format!("failed to decode metadata payload: {err}"),
                }
            })?;

        results.push(SearchResult {
            document: Document {
                id,
                content,
                metadata,
                embedding: None,
            },
            score,
        });
    }

    Ok(results)
}
