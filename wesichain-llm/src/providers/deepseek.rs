//! DeepSeek LLM client

use crate::openai_compatible::OpenAiCompatibleClient;
use crate::{LlmRequest, LlmResponse};
use futures::stream::BoxStream;
use std::time::Duration;
use wesichain_core::{Runnable, StreamEvent, WesichainError};

/// DeepSeek LLM client
#[derive(Clone)]
pub struct DeepSeekClient(OpenAiCompatibleClient);

impl DeepSeekClient {
    /// Create a new DeepSeek client with the given API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self(
            OpenAiCompatibleClient::builder()
                .base_url("https://api.deepseek.com")
                .expect("Valid URL")
                .api_key(api_key)
                .default_model("deepseek-chat")
                .timeout(Duration::from_secs(300))
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
impl Runnable<LlmRequest, LlmResponse> for DeepSeekClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        self.0.invoke(input).await
    }

    fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        self.0.stream(input)
    }
}
