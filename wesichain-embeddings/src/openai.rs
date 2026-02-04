use crate::EmbeddingProviderError;
use async_openai::config::OpenAIConfig;
use async_openai::types::CreateEmbeddingRequestArgs;
use async_openai::Client;
use async_trait::async_trait;
use wesichain_core::{Embedding, EmbeddingError};

#[derive(Clone)]
pub struct OpenAiEmbedding {
    client: Client<OpenAIConfig>,
    model: String,
    dimension: usize,
}

impl OpenAiEmbedding {
    pub fn new(client: Client<OpenAIConfig>, model: String, dimension: usize) -> Self {
        Self {
            client,
            model,
            dimension,
        }
    }
}

#[async_trait]
impl Embedding for OpenAiEmbedding {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let request = CreateEmbeddingRequestArgs::default()
            .model(&self.model)
            .input(text)
            .build()
            .map_err(|err| EmbeddingError::Other(Box::new(err)))?;

        let response = self
            .client
            .embeddings()
            .create(request)
            .await
            .map_err(|err| EmbeddingProviderError::Request(err.to_string()))?;

        let embedding = response
            .data
            .first()
            .map(|item| item.embedding.clone())
            .ok_or_else(|| {
                EmbeddingProviderError::InvalidResponse("missing embedding".to_string())
            })?;

        if embedding.len() != self.dimension {
            return Err(EmbeddingProviderError::InvalidResponse(format!(
                "expected embedding dimension {}, got {}",
                self.dimension,
                embedding.len()
            ))
            .into());
        }

        Ok(embedding)
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let inputs = texts.to_vec();
        let inputs_len = inputs.len();
        let request = CreateEmbeddingRequestArgs::default()
            .model(&self.model)
            .input(inputs)
            .build()
            .map_err(|err| EmbeddingError::Other(Box::new(err)))?;

        let response = self
            .client
            .embeddings()
            .create(request)
            .await
            .map_err(|err| EmbeddingProviderError::Request(err.to_string()))?;

        if response.data.len() != inputs_len {
            return Err(EmbeddingProviderError::InvalidResponse(format!(
                "expected {} embeddings, got {}",
                inputs_len,
                response.data.len()
            ))
            .into());
        }

        let mut out = Vec::with_capacity(response.data.len());
        for item in response.data {
            if item.embedding.len() != self.dimension {
                return Err(EmbeddingProviderError::InvalidResponse(format!(
                    "expected embedding dimension {}, got {}",
                    self.dimension,
                    item.embedding.len()
                ))
                .into());
            }
            out.push(item.embedding);
        }

        Ok(out)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}
