use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use wesichain_core::{Document, SearchResult, Value};

use crate::QdrantStoreError;

pub const CONTENT_PAYLOAD_KEY: &str = "__wesichain_content";

#[derive(Debug, Clone, Serialize)]
pub struct UpsertPointsRequest {
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeletePointsRequest {
    pub points: Vec<PointId>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchPointsRequest {
    pub vector: Vec<f32>,
    pub limit: usize,
    pub with_payload: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub result: T,
}

#[derive(Debug, Deserialize)]
pub struct ScoredPoint {
    pub id: PointId,
    pub score: f32,
    #[serde(default)]
    pub payload: JsonMap<String, JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PointId {
    String(String),
    Number(i64),
}

impl PointId {
    pub fn from_document_id(id: String) -> Result<Self, QdrantStoreError> {
        if id.trim().is_empty() {
            return Err(QdrantStoreError::InvalidDocumentId(id));
        }

        Ok(PointId::String(id))
    }

    pub fn as_string(&self) -> String {
        match self {
            PointId::String(value) => value.clone(),
            PointId::Number(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Point {
    pub id: PointId,
    pub vector: Vec<f32>,
    pub payload: JsonMap<String, JsonValue>,
}

pub fn doc_to_point(mut doc: Document) -> Result<Point, QdrantStoreError> {
    let id = PointId::from_document_id(doc.id)?;
    let vector = doc
        .embedding
        .take()
        .ok_or_else(|| QdrantStoreError::MissingEmbedding { id: id.as_string() })?;

    let mut payload = JsonMap::new();
    payload.insert(
        CONTENT_PAYLOAD_KEY.to_string(),
        JsonValue::String(doc.content),
    );

    for (key, value) in doc.metadata.drain() {
        payload.insert(key, value);
    }

    Ok(Point {
        id,
        vector,
        payload,
    })
}

pub fn scored_point_to_result(point: ScoredPoint) -> Result<SearchResult, QdrantStoreError> {
    let mut metadata: HashMap<String, Value> = HashMap::new();
    let point_id = point.id.as_string();
    let mut content: Option<String> = None;

    for (key, value) in point.payload {
        if key == CONTENT_PAYLOAD_KEY {
            let content_value =
                value
                    .as_str()
                    .ok_or_else(|| QdrantStoreError::InvalidContentPayloadType {
                        point_id: point_id.clone(),
                        expected: "string",
                        actual: value_type_name(&value),
                    })?;
            content = Some(content_value.to_string());
        } else {
            metadata.insert(key, value);
        }
    }

    let content = content.ok_or_else(|| QdrantStoreError::MissingContentPayload {
        point_id: point_id.clone(),
    })?;

    Ok(SearchResult {
        document: Document {
            id: point_id,
            content,
            metadata,
            embedding: None,
        },
        score: point.score,
    })
}

fn value_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}
