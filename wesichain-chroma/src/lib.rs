//! Chroma vector store integration for Wesichain.

use std::collections::HashMap;

use chroma::client::{ChromaAuthMethod, ChromaHttpClientError, ChromaHttpClientOptions};
use chroma::types::{
    BooleanOperator, CompositeExpression, IncludeList, MetadataComparison, MetadataExpression,
    MetadataSetValue, MetadataValue, PrimitiveOperator, QueryResponse, SetOperator, UpdateMetadata,
    UpdateMetadataValue, Where,
};
use chroma::{ChromaCollection, ChromaHttpClient};
use thiserror::Error;
use wesichain_core::{Document, MetadataFilter, SearchResult, StoreError, Value, VectorStore};

#[derive(Debug, Error)]
pub enum ChromaStoreError {
    #[error("invalid endpoint URL: {0}")]
    InvalidEndpoint(String),
    #[error("chroma client error: {0}")]
    Client(#[from] ChromaHttpClientError),
    #[error("document '{id}' is missing embedding")]
    MissingEmbedding { id: String },
    #[error("metadata key '{0}' is invalid for Chroma")]
    InvalidMetadataKey(String),
    #[error("unsupported metadata value for '{key}': {reason}")]
    UnsupportedMetadataValue { key: String, reason: String },
    #[error("unsupported metadata filter: {0}")]
    UnsupportedFilter(String),
}

impl From<ChromaStoreError> for StoreError {
    fn from(value: ChromaStoreError) -> Self {
        match value {
            ChromaStoreError::MissingEmbedding { id } => StoreError::Internal(Box::new(
                std::io::Error::other(format!("document '{id}' is missing embedding")),
            )),
            other => StoreError::Internal(Box::new(other)),
        }
    }
}

pub struct ChromaVectorStore {
    collection: ChromaCollection,
}

impl ChromaVectorStore {
    pub async fn new(
        endpoint: impl AsRef<str>,
        collection_name: impl Into<String>,
    ) -> Result<Self, ChromaStoreError> {
        let tenant_id =
            std::env::var("CHROMA_TENANT").unwrap_or_else(|_| "default_tenant".to_string());
        let database_name =
            std::env::var("CHROMA_DATABASE").unwrap_or_else(|_| "default_database".to_string());

        let options = ChromaHttpClientOptions {
            endpoint: endpoint
                .as_ref()
                .parse::<reqwest::Url>()
                .map_err(|err| ChromaStoreError::InvalidEndpoint(err.to_string()))?,
            auth_method: ChromaAuthMethod::None,
            tenant_id: Some(tenant_id),
            database_name: Some(database_name),
            ..Default::default()
        };

        let client = ChromaHttpClient::new(options);
        Self::with_client(client, collection_name).await
    }

    pub async fn with_client(
        client: ChromaHttpClient,
        collection_name: impl Into<String>,
    ) -> Result<Self, ChromaStoreError> {
        let collection = client
            .get_or_create_collection(collection_name.into(), None, None)
            .await?;
        Ok(Self { collection })
    }

    pub fn collection_name(&self) -> &str {
        self.collection.name()
    }
}

#[async_trait::async_trait]
impl VectorStore for ChromaVectorStore {
    async fn add(&self, docs: Vec<Document>) -> Result<(), StoreError> {
        if docs.is_empty() {
            return Ok(());
        }

        let mut ids = Vec::with_capacity(docs.len());
        let mut embeddings = Vec::with_capacity(docs.len());
        let mut documents = Vec::with_capacity(docs.len());
        let mut metadatas = Vec::with_capacity(docs.len());
        let mut expected_dimension: Option<usize> = None;

        for mut doc in docs {
            if doc.id.trim().is_empty() {
                return Err(StoreError::InvalidId(doc.id));
            }

            let embedding = doc
                .embedding
                .take()
                .ok_or_else(|| ChromaStoreError::MissingEmbedding { id: doc.id.clone() })
                .map_err(StoreError::from)?;

            match expected_dimension {
                Some(expected) if expected != embedding.len() => {
                    return Err(StoreError::DimensionMismatch {
                        expected,
                        got: embedding.len(),
                    });
                }
                None => expected_dimension = Some(embedding.len()),
                _ => {}
            }

            let metadata = to_update_metadata(doc.metadata).map_err(StoreError::from)?;

            ids.push(doc.id);
            embeddings.push(embedding);
            documents.push(Some(doc.content));
            metadatas.push(Some(metadata));
        }

        self.collection
            .upsert(ids, embeddings, Some(documents), None, Some(metadatas))
            .await
            .map_err(ChromaStoreError::from)
            .map_err(StoreError::from)
            .map(|_| ())
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, StoreError> {
        if query_embedding.is_empty() || top_k == 0 {
            return Ok(Vec::new());
        }

        let where_clause = filter
            .map(filter_to_where)
            .transpose()
            .map_err(StoreError::from)?;

        let top_k_u32 = top_k.min(u32::MAX as usize) as u32;

        let response = self
            .collection
            .query(
                vec![query_embedding.to_vec()],
                Some(top_k_u32),
                where_clause,
                None,
                Some(IncludeList::default_query()),
            )
            .await
            .map_err(ChromaStoreError::from)
            .map_err(StoreError::from)?;

        Ok(query_response_to_results(response))
    }

    async fn delete(&self, ids: &[String]) -> Result<(), StoreError> {
        if ids.is_empty() {
            return Ok(());
        }

        self.collection
            .delete(Some(ids.to_vec()), None)
            .await
            .map_err(ChromaStoreError::from)
            .map_err(StoreError::from)
            .map(|_| ())
    }
}

fn query_response_to_results(response: QueryResponse) -> Vec<SearchResult> {
    let ids = response.ids.into_iter().next().unwrap_or_default();
    let documents = response
        .documents
        .and_then(|mut batches| batches.pop())
        .unwrap_or_default();
    let metadatas = response
        .metadatas
        .and_then(|mut batches| batches.pop())
        .unwrap_or_default();
    let distances = response
        .distances
        .and_then(|mut batches| batches.pop())
        .unwrap_or_default();

    ids.into_iter()
        .enumerate()
        .map(|(idx, id)| {
            let content = documents
                .get(idx)
                .and_then(|value| value.clone())
                .unwrap_or_default();

            let metadata: HashMap<String, Value> = metadatas
                .get(idx)
                .cloned()
                .flatten()
                .unwrap_or_default()
                .into_iter()
                .map(|(key, value)| (key, Value::from(value)))
                .collect();

            let score = distances
                .get(idx)
                .copied()
                .flatten()
                .map(|distance| -distance)
                .unwrap_or(0.0);

            SearchResult {
                document: Document {
                    id,
                    content,
                    metadata,
                    embedding: None,
                },
                score,
            }
        })
        .collect()
}

fn filter_to_where(filter: &MetadataFilter) -> Result<Where, ChromaStoreError> {
    match filter {
        MetadataFilter::Eq(key, value) => Ok(Where::Metadata(MetadataExpression {
            key: key.clone(),
            comparison: MetadataComparison::Primitive(
                PrimitiveOperator::Equal,
                json_to_metadata_value(key, value)?,
            ),
        })),
        MetadataFilter::In(key, values) => Ok(Where::Metadata(MetadataExpression {
            key: key.clone(),
            comparison: MetadataComparison::Set(SetOperator::In, json_to_set_value(key, values)?),
        })),
        MetadataFilter::Range { key, min, max } => {
            let mut children = Vec::new();

            if let Some(min_value) = min {
                children.push(Where::Metadata(MetadataExpression {
                    key: key.clone(),
                    comparison: MetadataComparison::Primitive(
                        PrimitiveOperator::GreaterThanOrEqual,
                        json_number_to_metadata_value(key, min_value)?,
                    ),
                }));
            }

            if let Some(max_value) = max {
                children.push(Where::Metadata(MetadataExpression {
                    key: key.clone(),
                    comparison: MetadataComparison::Primitive(
                        PrimitiveOperator::LessThanOrEqual,
                        json_number_to_metadata_value(key, max_value)?,
                    ),
                }));
            }

            match children.len() {
                0 => Err(ChromaStoreError::UnsupportedFilter(format!(
                    "range filter for '{key}' has neither min nor max"
                ))),
                1 => Ok(children.remove(0)),
                _ => Ok(Where::Composite(CompositeExpression {
                    operator: BooleanOperator::And,
                    children,
                })),
            }
        }
        MetadataFilter::All(filters) => {
            if filters.is_empty() {
                return Err(ChromaStoreError::UnsupportedFilter(
                    "empty all(...) filter".to_string(),
                ));
            }

            let mut children = Vec::with_capacity(filters.len());
            for nested in filters {
                children.push(filter_to_where(nested)?);
            }

            Ok(Where::Composite(CompositeExpression {
                operator: BooleanOperator::And,
                children,
            }))
        }
        MetadataFilter::Any(filters) => {
            if filters.is_empty() {
                return Err(ChromaStoreError::UnsupportedFilter(
                    "empty any(...) filter".to_string(),
                ));
            }

            let mut children = Vec::with_capacity(filters.len());
            for nested in filters {
                children.push(filter_to_where(nested)?);
            }

            Ok(Where::Composite(CompositeExpression {
                operator: BooleanOperator::Or,
                children,
            }))
        }
    }
}

fn to_update_metadata(
    metadata: HashMap<String, Value>,
) -> Result<UpdateMetadata, ChromaStoreError> {
    let mut out = UpdateMetadata::new();

    for (key, value) in metadata {
        if key.starts_with('$') || key.starts_with('#') {
            return Err(ChromaStoreError::InvalidMetadataKey(key));
        }

        out.insert(key.clone(), json_to_update_metadata_value(&key, &value)?);
    }

    Ok(out)
}

fn json_to_update_metadata_value(
    key: &str,
    value: &Value,
) -> Result<UpdateMetadataValue, ChromaStoreError> {
    match value {
        Value::Bool(value) => Ok(UpdateMetadataValue::Bool(*value)),
        Value::Number(value) => {
            if let Some(int_value) = json_number_to_i64(value) {
                Ok(UpdateMetadataValue::Int(int_value))
            } else if let Some(float_value) = value.as_f64() {
                Ok(UpdateMetadataValue::Float(float_value))
            } else {
                Err(unsupported_metadata(key, "number cannot be represented"))
            }
        }
        Value::String(value) => Ok(UpdateMetadataValue::Str(value.clone())),
        Value::Array(values) => json_array_to_update_metadata_value(key, values),
        Value::Null => Err(unsupported_metadata(key, "null is not supported")),
        Value::Object(_) => Err(unsupported_metadata(
            key,
            "object metadata is not supported by Chroma",
        )),
    }
}

fn json_array_to_update_metadata_value(
    key: &str,
    values: &[Value],
) -> Result<UpdateMetadataValue, ChromaStoreError> {
    if values.is_empty() {
        return Err(unsupported_metadata(key, "empty arrays are not supported"));
    }

    if values.iter().all(Value::is_boolean) {
        let out: Vec<bool> = values.iter().filter_map(Value::as_bool).collect();
        return Ok(UpdateMetadataValue::BoolArray(out));
    }

    if values.iter().all(is_json_integer) {
        let mut out = Vec::with_capacity(values.len());
        for value in values {
            let Some(number) = value.as_number() else {
                return Err(unsupported_metadata(key, "invalid integer array value"));
            };
            let Some(int_value) = json_number_to_i64(number) else {
                return Err(unsupported_metadata(key, "integer value out of range"));
            };
            out.push(int_value);
        }
        return Ok(UpdateMetadataValue::IntArray(out));
    }

    if values.iter().all(Value::is_number) {
        let mut out = Vec::with_capacity(values.len());
        for value in values {
            let Some(float_value) = value.as_f64() else {
                return Err(unsupported_metadata(key, "invalid float array value"));
            };
            out.push(float_value);
        }
        return Ok(UpdateMetadataValue::FloatArray(out));
    }

    if values.iter().all(Value::is_string) {
        let out: Vec<String> = values
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect();
        return Ok(UpdateMetadataValue::StringArray(out));
    }

    Err(unsupported_metadata(
        key,
        "arrays must be homogeneous (bool/int/float/string)",
    ))
}

fn json_to_metadata_value(key: &str, value: &Value) -> Result<MetadataValue, ChromaStoreError> {
    match value {
        Value::Bool(value) => Ok(MetadataValue::Bool(*value)),
        Value::Number(value) => {
            if let Some(int_value) = json_number_to_i64(value) {
                Ok(MetadataValue::Int(int_value))
            } else if let Some(float_value) = value.as_f64() {
                Ok(MetadataValue::Float(float_value))
            } else {
                Err(unsupported_metadata(key, "number cannot be represented"))
            }
        }
        Value::String(value) => Ok(MetadataValue::Str(value.clone())),
        Value::Array(values) => {
            if values.is_empty() {
                return Err(unsupported_metadata(key, "empty arrays are not supported"));
            }

            if values.iter().all(Value::is_boolean) {
                let out: Vec<bool> = values.iter().filter_map(Value::as_bool).collect();
                return Ok(MetadataValue::BoolArray(out));
            }

            if values.iter().all(is_json_integer) {
                let mut out = Vec::with_capacity(values.len());
                for value in values {
                    let Some(number) = value.as_number() else {
                        return Err(unsupported_metadata(key, "invalid integer array value"));
                    };
                    let Some(int_value) = json_number_to_i64(number) else {
                        return Err(unsupported_metadata(key, "integer value out of range"));
                    };
                    out.push(int_value);
                }
                return Ok(MetadataValue::IntArray(out));
            }

            if values.iter().all(Value::is_number) {
                let mut out = Vec::with_capacity(values.len());
                for value in values {
                    let Some(float_value) = value.as_f64() else {
                        return Err(unsupported_metadata(key, "invalid float array value"));
                    };
                    out.push(float_value);
                }
                return Ok(MetadataValue::FloatArray(out));
            }

            if values.iter().all(Value::is_string) {
                let out: Vec<String> = values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect();
                return Ok(MetadataValue::StringArray(out));
            }

            Err(unsupported_metadata(
                key,
                "arrays must be homogeneous (bool/int/float/string)",
            ))
        }
        Value::Null => Err(unsupported_metadata(key, "null is not supported")),
        Value::Object(_) => Err(unsupported_metadata(
            key,
            "object metadata is not supported by Chroma",
        )),
    }
}

fn json_to_set_value(key: &str, values: &[Value]) -> Result<MetadataSetValue, ChromaStoreError> {
    if values.is_empty() {
        return Err(unsupported_metadata(key, "set filter cannot be empty"));
    }

    if values.iter().all(Value::is_boolean) {
        let out = values
            .iter()
            .filter_map(Value::as_bool)
            .collect::<Vec<bool>>();
        return Ok(MetadataSetValue::Bool(out));
    }

    if values.iter().all(is_json_integer) {
        let mut out = Vec::with_capacity(values.len());
        for value in values {
            let Some(number) = value.as_number() else {
                return Err(unsupported_metadata(key, "invalid integer set value"));
            };
            let Some(int_value) = json_number_to_i64(number) else {
                return Err(unsupported_metadata(key, "integer value out of range"));
            };
            out.push(int_value);
        }
        return Ok(MetadataSetValue::Int(out));
    }

    if values.iter().all(Value::is_number) {
        let mut out = Vec::with_capacity(values.len());
        for value in values {
            let Some(float_value) = value.as_f64() else {
                return Err(unsupported_metadata(key, "invalid float set value"));
            };
            out.push(float_value);
        }
        return Ok(MetadataSetValue::Float(out));
    }

    if values.iter().all(Value::is_string) {
        let out = values
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect::<Vec<String>>();
        return Ok(MetadataSetValue::Str(out));
    }

    Err(unsupported_metadata(
        key,
        "set values must be homogeneous (bool/int/float/string)",
    ))
}

fn json_number_to_metadata_value(
    key: &str,
    value: &Value,
) -> Result<MetadataValue, ChromaStoreError> {
    match value {
        Value::Number(number) => {
            if let Some(int_value) = json_number_to_i64(number) {
                Ok(MetadataValue::Int(int_value))
            } else if let Some(float_value) = number.as_f64() {
                Ok(MetadataValue::Float(float_value))
            } else {
                Err(unsupported_metadata(key, "number cannot be represented"))
            }
        }
        _ => Err(unsupported_metadata(key, "range bounds must be numeric")),
    }
}

fn json_number_to_i64(number: &serde_json::Number) -> Option<i64> {
    if let Some(value) = number.as_i64() {
        return Some(value);
    }

    number.as_u64().and_then(|value| i64::try_from(value).ok())
}

fn is_json_integer(value: &Value) -> bool {
    value
        .as_number()
        .map(|number| number.as_i64().is_some() || number.as_u64().is_some())
        .unwrap_or(false)
}

fn unsupported_metadata(key: &str, reason: &str) -> ChromaStoreError {
    ChromaStoreError::UnsupportedMetadataValue {
        key: key.to_string(),
        reason: reason.to_string(),
    }
}
