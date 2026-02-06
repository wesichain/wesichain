use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
pub struct PineconeVector {
    pub id: String,
    pub values: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpsertRequest {
    pub vectors: Vec<PineconeVector>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct QueryRequest {
    pub vector: Vec<f32>,
    pub top_k: usize,
    pub include_metadata: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct QueryMatch {
    pub id: String,
    pub score: f32,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct QueryResponse {
    #[serde(default)]
    pub matches: Vec<QueryMatch>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeleteRequest {
    pub ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IndexStatsResponse {
    pub dimension: Option<usize>,
}
