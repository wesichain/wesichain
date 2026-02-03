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
            .map_err(|err| EmbeddingError::Provider(err.to_string()))?;

        response
            .data
            .get(0)
            .map(|item| item.embedding.clone())
            .ok_or_else(|| EmbeddingError::InvalidResponse("missing embedding".to_string()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let inputs = texts.to_vec();
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
            .map_err(|err| EmbeddingError::Provider(err.to_string()))?;

        Ok(response.data.into_iter().map(|item| item.embedding).collect())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}
