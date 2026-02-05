//! OpenAI LLM client

use crate::openai_compatible::OpenAiCompatibleClient;
use crate::{LlmRequest, LlmResponse};
use futures::stream::BoxStream;
use std::time::Duration;
use wesichain_core::{Runnable, StreamEvent, WesichainError};

/// OpenAI LLM client
#[derive(Clone)]
pub struct OpenAiClient(OpenAiCompatibleClient);

impl OpenAiClient {
    /// Create a new OpenAI client with the given API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self(
            OpenAiCompatibleClient::builder()
                .base_url("https://api.openai.com")
                .expect("Valid URL")
                .api_key(api_key)
                .default_model("gpt-4o-mini")
                .timeout(Duration::from_secs(60))
                .build()
                .expect("Valid config"),
        )
    }

    /// Set the model to use
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.0.set_default_model(model);
        self
    }
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for OpenAiClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        self.0.invoke(input).await
    }

    fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        self.0.stream(input)
    }
}
