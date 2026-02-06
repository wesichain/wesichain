use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SparseValues {
    pub indices: Vec<u32>,
    pub values: Vec<f32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PineconeVector {
    pub id: String,
    pub values: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparse_values: Option<SparseValues>,
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{PineconeVector, SparseValues};

    #[test]
    fn pinecone_vector_omits_sparse_values_when_absent() {
        let vector = PineconeVector {
            id: "doc-1".to_string(),
            values: vec![0.1, 0.2],
            sparse_values: None,
            metadata: Some(json!({"source": "guide"})),
        };

        let serialized = serde_json::to_value(vector).unwrap();
        assert!(serialized.get("sparse_values").is_none());
    }

    #[test]
    fn pinecone_vector_serializes_sparse_values_when_present() {
        let vector = PineconeVector {
            id: "doc-1".to_string(),
            values: vec![0.1, 0.2],
            sparse_values: Some(SparseValues {
                indices: vec![1, 4],
                values: vec![0.7, 0.2],
            }),
            metadata: Some(json!({"source": "guide"})),
        };

        let serialized = serde_json::to_value(vector).unwrap();
        let sparse = serialized.get("sparse_values").unwrap();
        assert_eq!(sparse.get("indices"), Some(&json!([1, 4])));

        let values = sparse.get("values").and_then(|v| v.as_array()).unwrap();
        assert_eq!(values.len(), 2);
        assert!((values[0].as_f64().unwrap() - 0.7).abs() < 1e-6);
        assert!((values[1].as_f64().unwrap() - 0.2).abs() < 1e-6);
    }
}
