//! Weaviate vector store integration for Wesichain.

mod config;
mod error;
pub mod filter;
pub mod mapper;

use std::fmt;

use mapper::{
    build_near_vector_query, class_schema_request, doc_to_object, graphql_hits_to_results,
    GraphQlRequest, GraphQlResponse,
};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use wesichain_core::{Document, MetadataFilter, SearchResult, StoreError, VectorStore};

use crate::filter::to_weaviate_filter;

pub use config::WeaviateStoreBuilder;
pub use error::WeaviateStoreError;

#[derive(Clone)]
pub struct WeaviateVectorStore {
    client: reqwest::Client,
    base_url: String,
    class_name: String,
    api_key: Option<String>,
    auto_create_class: bool,
}

impl fmt::Debug for WeaviateVectorStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let api_key = if self.api_key.is_some() {
            "<redacted>"
        } else {
            "<none>"
        };

        f.debug_struct("WeaviateVectorStore")
            .field("base_url", &self.base_url)
            .field("class_name", &self.class_name)
            .field("api_key", &api_key)
            .field("auto_create_class", &self.auto_create_class)
            .finish()
    }
}

impl WeaviateVectorStore {
    pub fn builder() -> WeaviateStoreBuilder {
        WeaviateStoreBuilder::new()
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn class_name(&self) -> &str {
        &self.class_name
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

    pub fn auto_create_class(&self) -> bool {
        self.auto_create_class
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
            request.header("Authorization", format!("Bearer {api_key}"))
        } else {
            request
        }
    }

    async fn send_json(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<JsonValue, WeaviateStoreError> {
        let response = request.send().await.map_err(WeaviateStoreError::from)?;
        let status = response.status();
        let body = response.text().await.map_err(WeaviateStoreError::from)?;

        if !status.is_success() {
            return Err(self.http_error_from_response(status.as_u16(), &body));
        }

        if body.trim().is_empty() {
            return Ok(JsonValue::Null);
        }

        serde_json::from_str(&body).map_err(|err| WeaviateStoreError::InvalidResponse {
            message: format!("failed to decode weaviate response body: {err}"),
        })
    }

    async fn create_class_schema(&self) -> Result<(), WeaviateStoreError> {
        let schema = class_schema_request(&self.class_name);
        match self
            .send_json(
                self.request_builder(reqwest::Method::POST, "v1/schema")
                    .json(&schema),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(WeaviateStoreError::HttpStatus { status, message })
                if (status == 409 || status == 422)
                    && is_class_already_exists_message(&message) =>
            {
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    async fn add_once(&self, docs: Vec<Document>) -> Result<(), WeaviateStoreError> {
        let mut objects = Vec::with_capacity(docs.len());
        let mut expected_dimension: Option<usize> = None;
        for doc in docs {
            let object = doc_to_object(doc, &self.class_name)?;
            match expected_dimension {
                Some(expected) if expected != object.vector.len() => {
                    return Err(WeaviateStoreError::InvalidResponse {
                        message: format!(
                            "dimension mismatch in batch: expected {expected}, got {}",
                            object.vector.len()
                        ),
                    });
                }
                None => expected_dimension = Some(object.vector.len()),
                _ => {}
            }
            objects.push(object);
        }

        for object in objects {
            let request = self
                .request_builder(reqwest::Method::POST, "v1/objects")
                .json(&object);

            let _ = self.send_json(request).await?;
        }

        Ok(())
    }

    fn http_error_from_response(&self, status: u16, body: &str) -> WeaviateStoreError {
        let message = weaviate_error_message(body);
        if is_class_not_found_message(&message) {
            return WeaviateStoreError::ClassNotFound {
                class_name: self.class_name.clone(),
                message,
            };
        }

        WeaviateStoreError::HttpStatus { status, message }
    }
}

#[async_trait::async_trait]
impl VectorStore for WeaviateVectorStore {
    async fn add(&self, docs: Vec<Document>) -> Result<(), StoreError> {
        if docs.is_empty() {
            return Ok(());
        }

        match self.add_once(docs.clone()).await {
            Ok(()) => Ok(()),
            Err(WeaviateStoreError::ClassNotFound { .. }) if self.auto_create_class => {
                self.create_class_schema().await.map_err(StoreError::from)?;
                self.add_once(docs).await.map_err(StoreError::from)
            }
            Err(error) => Err(StoreError::from(error)),
        }
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
            .map(to_weaviate_filter)
            .transpose()
            .map_err(StoreError::from)?;

        let query = build_near_vector_query(
            &self.class_name,
            query_embedding,
            top_k,
            where_clause.as_deref(),
        );
        let response = self
            .send_json(
                self.request_builder(reqwest::Method::POST, "v1/graphql")
                    .json(&GraphQlRequest { query }),
            )
            .await
            .map_err(StoreError::from)?;

        let gql_response: GraphQlResponse = serde_json::from_value(response).map_err(|err| {
            StoreError::from(WeaviateStoreError::InvalidResponse {
                message: format!("failed to decode GraphQL envelope: {err}"),
            })
        })?;

        if let Some(first_error) = gql_response.errors.first() {
            let message = first_error.message.clone();
            if is_class_not_found_message(&message) {
                return Err(StoreError::from(WeaviateStoreError::ClassNotFound {
                    class_name: self.class_name.clone(),
                    message,
                }));
            }

            return Err(StoreError::from(WeaviateStoreError::InvalidResponse {
                message,
            }));
        }

        let data = gql_response.data.ok_or_else(|| {
            StoreError::from(WeaviateStoreError::InvalidResponse {
                message: "missing GraphQL data in response".to_string(),
            })
        })?;

        let mut results =
            graphql_hits_to_results(data, &self.class_name).map_err(StoreError::from)?;
        results.sort_by(|left, right| right.score.total_cmp(&left.score));
        Ok(results)
    }

    async fn delete(&self, ids: &[String]) -> Result<(), StoreError> {
        if ids.is_empty() {
            return Ok(());
        }

        for id in ids {
            if id.trim().is_empty() {
                return Err(StoreError::InvalidId(id.clone()));
            }

            let encoded_id = urlencoding::encode(id);
            let path = format!("v1/objects/{}/{}", self.class_name, encoded_id);
            let _ = self
                .send_json(self.request_builder(reqwest::Method::DELETE, &path))
                .await
                .map_err(StoreError::from)?;
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct WeaviateErrorEnvelope {
    #[serde(default)]
    error: Vec<WeaviateErrorMessage>,
}

#[derive(Debug, Deserialize)]
struct WeaviateErrorMessage {
    message: String,
}

fn weaviate_error_message(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "unknown weaviate error".to_string();
    }

    serde_json::from_str::<WeaviateErrorEnvelope>(trimmed)
        .ok()
        .and_then(|envelope| envelope.error.into_iter().next().map(|entry| entry.message))
        .unwrap_or_else(|| trimmed.to_string())
}

fn is_class_not_found_message(message: &str) -> bool {
    let normalized = message.to_lowercase();
    normalized.contains("class") && normalized.contains("not found")
}

fn is_class_already_exists_message(message: &str) -> bool {
    let normalized = message.to_lowercase();
    normalized.contains("class")
        && (normalized.contains("already exists") || normalized.contains("already exist"))
}
