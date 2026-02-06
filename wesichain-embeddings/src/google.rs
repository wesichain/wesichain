use crate::EmbeddingProviderError;
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use wesichain_core::{Embedding, EmbeddingError};

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";

#[derive(Clone)]
pub struct GoogleEmbedding {
    base_url: String,
    api_key: String,
    model: String,
    dimension: usize,
    task_type: Option<String>,
    http: Client,
}

impl GoogleEmbedding {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>, dimension: usize) -> Self {
        Self {
            base_url: GEMINI_BASE_URL.to_string(),
            api_key: api_key.into(),
            model: model.into(),
            dimension,
            task_type: None,
            http: Client::new(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn with_task_type(mut self, task_type: impl Into<String>) -> Self {
        self.task_type = Some(task_type.into());
        self
    }

    fn model_name(&self) -> &str {
        self.model
            .strip_prefix("models/")
            .unwrap_or(self.model.as_str())
    }

    fn embed_url(&self) -> String {
        format!(
            "{}/v1beta/models/{}:embedContent",
            self.base_url.trim_end_matches('/'),
            self.model_name()
        )
    }

    fn batch_embed_url(&self) -> String {
        format!(
            "{}/v1beta/models/{}:batchEmbedContents",
            self.base_url.trim_end_matches('/'),
            self.model_name()
        )
    }
}

#[derive(Debug, Serialize)]
struct Part {
    text: String,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EmbedContentRequest {
    content: Content,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct BatchEmbedContentsRequest {
    requests: Vec<EmbedContentRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmbedContentResponse {
    embedding: ContentEmbedding,
}

#[derive(Debug, Deserialize)]
struct BatchEmbedContentsResponse {
    embeddings: Vec<ContentEmbedding>,
}

#[derive(Debug, Deserialize)]
struct ContentEmbedding {
    #[serde(alias = "value")]
    values: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct GoogleErrorResponse {
    error: GoogleErrorDetail,
}

#[derive(Debug, Deserialize)]
struct GoogleErrorDetail {
    message: String,
}

#[async_trait]
impl Embedding for GoogleEmbedding {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let request = EmbedContentRequest {
            content: Content {
                parts: vec![Part {
                    text: text.to_string(),
                }],
            },
            task_type: self.task_type.clone(),
        };

        let response = self
            .http
            .post(self.embed_url())
            .query(&[("key", self.api_key.as_str())])
            .json(&request)
            .send()
            .await
            .map_err(|err| EmbeddingProviderError::Request(err.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            if status == StatusCode::TOO_MANY_REQUESTS {
                return Err(EmbeddingError::RateLimited { retry_after: None });
            }
            let body = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<GoogleErrorResponse>(&body)
                .map(|e| e.error.message)
                .unwrap_or_else(|_| format!("HTTP {}: {}", status, body));
            return Err(EmbeddingProviderError::Request(message).into());
        }

        let response = response
            .json::<EmbedContentResponse>()
            .await
            .map_err(|err| EmbeddingProviderError::Request(err.to_string()))?;

        if response.embedding.values.len() != self.dimension {
            return Err(EmbeddingProviderError::InvalidResponse(format!(
                "expected embedding dimension {}, got {}",
                self.dimension,
                response.embedding.values.len()
            ))
            .into());
        }

        Ok(response.embedding.values)
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let requests = texts
            .iter()
            .map(|text| EmbedContentRequest {
                content: Content {
                    parts: vec![Part { text: text.clone() }],
                },
                task_type: self.task_type.clone(),
            })
            .collect();

        let request = BatchEmbedContentsRequest { requests };

        let response = self
            .http
            .post(self.batch_embed_url())
            .query(&[("key", self.api_key.as_str())])
            .json(&request)
            .send()
            .await
            .map_err(|err| EmbeddingProviderError::Request(err.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            if status == StatusCode::TOO_MANY_REQUESTS {
                return Err(EmbeddingError::RateLimited { retry_after: None });
            }
            let body = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<GoogleErrorResponse>(&body)
                .map(|e| e.error.message)
                .unwrap_or_else(|_| format!("HTTP {}: {}", status, body));
            return Err(EmbeddingProviderError::Request(message).into());
        }

        let response = response
            .json::<BatchEmbedContentsResponse>()
            .await
            .map_err(|err| EmbeddingProviderError::Request(err.to_string()))?;

        if response.embeddings.len() != texts.len() {
            return Err(EmbeddingProviderError::InvalidResponse(format!(
                "expected {} embeddings, got {}",
                texts.len(),
                response.embeddings.len()
            ))
            .into());
        }

        let mut output = Vec::with_capacity(response.embeddings.len());
        for embedding in response.embeddings {
            if embedding.values.len() != self.dimension {
                return Err(EmbeddingProviderError::InvalidResponse(format!(
                    "expected embedding dimension {}, got {}",
                    self.dimension,
                    embedding.values.len()
                ))
                .into());
            }
            output.push(embedding.values);
        }

        Ok(output)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}
