use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::EmbeddingProviderError;
use wesichain_core::{Embedding, EmbeddingError};

#[derive(Clone)]
pub struct OllamaEmbedding {
    base_url: String,
    model: String,
    dimension: usize,
    http: Client,
}

impl OllamaEmbedding {
    pub fn new(base_url: String, model: String, dimension: usize) -> Self {
        Self {
            base_url,
            model,
            dimension,
            http: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct OllamaEmbeddingRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f32>,
}

#[async_trait]
impl Embedding for OllamaEmbedding {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let url = format!("{}/api/embeddings", self.base_url.trim_end_matches('/'));
        let req = OllamaEmbeddingRequest {
            model: self.model.clone(),
            prompt: text.to_string(),
        };
        let response: OllamaEmbeddingResponse = self
            .http
            .post(url)
            .json(&req)
            .send()
            .await
            .map_err(|err| EmbeddingProviderError::Request(err.to_string()))?
            .error_for_status()
            .map_err(|err| EmbeddingProviderError::Request(err.to_string()))?
            .json()
            .await
            .map_err(|err| EmbeddingProviderError::Request(err.to_string()))?;

        if response.embedding.len() != self.dimension {
            return Err(EmbeddingProviderError::InvalidResponse(format!(
                "expected embedding dimension {}, got {}",
                self.dimension,
                response.embedding.len()
            ))
            .into());
        }

        Ok(response.embedding)
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            out.push(self.embed(text).await?);
        }
        Ok(out)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}
