//! Qdrant vector store integration for Wesichain.

mod config;
mod error;
pub mod filter;
pub mod mapper;

use std::fmt;

pub use config::QdrantStoreBuilder;
pub use error::QdrantStoreError;
use filter::{qdrant_filter_to_payload, to_qdrant_filter};
use mapper::{
    doc_to_point, scored_point_to_result, ApiResponse, DeletePointsRequest, PointId, ScoredPoint,
    SearchPointsRequest, UpsertPointsRequest,
};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use wesichain_core::{Document, MetadataFilter, SearchResult, StoreError, VectorStore};

#[derive(Clone)]
pub struct QdrantVectorStore {
    client: reqwest::Client,
    base_url: String,
    collection: String,
    api_key: Option<String>,
}

impl fmt::Debug for QdrantVectorStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let api_key = if self.api_key.is_some() {
            "<redacted>"
        } else {
            "<none>"
        };

        f.debug_struct("QdrantVectorStore")
            .field("base_url", &self.base_url)
            .field("collection", &self.collection)
            .field("api_key", &api_key)
            .finish()
    }
}

impl QdrantVectorStore {
    pub fn builder() -> QdrantStoreBuilder {
        QdrantStoreBuilder::new()
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn collection(&self) -> &str {
        &self.collection
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    pub async fn scored_search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, StoreError> {
        <Self as VectorStore>::search(self, query_embedding, top_k, filter).await
    }

    fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn request_builder(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let request = self.client.request(method, self.endpoint(path));

        if let Some(api_key) = self.api_key() {
            request.header("api-key", api_key)
        } else {
            request
        }
    }

    async fn send_and_decode<T: for<'de> Deserialize<'de>>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, QdrantStoreError> {
        let response = request.send().await.map_err(QdrantStoreError::from)?;
        let status = response.status();
        let body = response.text().await.map_err(QdrantStoreError::from)?;

        if !status.is_success() {
            return Err(self.http_error_from_response(status.as_u16(), &body));
        }

        serde_json::from_str(&body).map_err(|err| QdrantStoreError::InvalidResponse {
            message: format!("failed to decode qdrant response body: {err}"),
        })
    }

    fn http_error_from_response(&self, status: u16, body: &str) -> QdrantStoreError {
        let message = qdrant_error_message(body);
        if status == 404 && message.to_lowercase().contains("collection") {
            return QdrantStoreError::CollectionNotFound {
                collection: self.collection.clone(),
                message,
            };
        }

        QdrantStoreError::HttpStatus { status, message }
    }
}

#[async_trait::async_trait]
impl VectorStore for QdrantVectorStore {
    async fn add(&self, docs: Vec<Document>) -> Result<(), StoreError> {
        if docs.is_empty() {
            return Ok(());
        }

        let mut points = Vec::with_capacity(docs.len());
        let mut expected_dimension: Option<usize> = None;

        for doc in docs {
            let point = doc_to_point(doc).map_err(StoreError::from)?;
            match expected_dimension {
                Some(expected) if expected != point.vector.len() => {
                    return Err(StoreError::DimensionMismatch {
                        expected,
                        got: point.vector.len(),
                    });
                }
                None => expected_dimension = Some(point.vector.len()),
                _ => {}
            }
            points.push(point);
        }

        let request = UpsertPointsRequest { points };
        let _: ApiResponse<JsonValue> = self
            .send_and_decode(
                self.request_builder(
                    reqwest::Method::PUT,
                    &format!("collections/{}/points?wait=true", self.collection),
                )
                .json(&request),
            )
            .await
            .map_err(StoreError::from)?;

        Ok(())
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

        let qdrant_filter = match filter {
            Some(filter) => {
                let translated = to_qdrant_filter(filter).map_err(StoreError::from)?;
                Some(qdrant_filter_to_payload(&translated).map_err(StoreError::from)?)
            }
            None => None,
        };

        let request = SearchPointsRequest {
            vector: query_embedding.to_vec(),
            limit: top_k,
            with_payload: true,
            filter: qdrant_filter,
        };

        let response: ApiResponse<Vec<ScoredPoint>> = self
            .send_and_decode(
                self.request_builder(
                    reqwest::Method::POST,
                    &format!("collections/{}/points/search", self.collection),
                )
                .json(&request),
            )
            .await
            .map_err(StoreError::from)?;

        let mut results = response
            .result
            .into_iter()
            .map(scored_point_to_result)
            .collect::<Result<Vec<SearchResult>, QdrantStoreError>>()
            .map_err(StoreError::from)?;

        results.sort_by(|left, right| right.score.total_cmp(&left.score));
        Ok(results)
    }

    async fn delete(&self, ids: &[String]) -> Result<(), StoreError> {
        if ids.is_empty() {
            return Ok(());
        }

        let points = ids
            .iter()
            .cloned()
            .map(PointId::from_document_id)
            .collect::<Result<Vec<PointId>, QdrantStoreError>>()
            .map_err(StoreError::from)?;

        let request = DeletePointsRequest { points };
        let _: ApiResponse<JsonValue> = self
            .send_and_decode(
                self.request_builder(
                    reqwest::Method::POST,
                    &format!("collections/{}/points/delete?wait=true", self.collection),
                )
                .json(&request),
            )
            .await
            .map_err(StoreError::from)?;

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct QdrantErrorEnvelope {
    status: QdrantErrorStatus,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum QdrantErrorStatus {
    Message(String),
    Structured { error: String },
}

fn qdrant_error_message(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "unknown qdrant error".to_string();
    }

    serde_json::from_str::<QdrantErrorEnvelope>(trimmed)
        .map(|envelope| match envelope.status {
            QdrantErrorStatus::Message(message) => message,
            QdrantErrorStatus::Structured { error } => error,
        })
        .unwrap_or_else(|_| trimmed.to_string())
}
