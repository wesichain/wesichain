//! Together AI LLM client.
//!
//! Together AI is fully OpenAI-compatible.  Wraps [`OpenAiCompatibleClient`]
//! with the Together AI base URL.
//!
//! # Example
//! ```ignore
//! use wesichain_llm::providers::together::TogetherClient;
//!
//! let llm = TogetherClient::new(
//!     std::env::var("TOGETHER_API_KEY").unwrap(),
//!     "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo",
//! );
//! ```

use std::time::Duration;

use futures::stream::BoxStream;
use wesichain_core::{Runnable, StreamEvent, WesichainError};

use crate::openai_compatible::OpenAiCompatibleClient;
use crate::{LlmRequest, LlmResponse};

const TOGETHER_BASE_URL: &str = "https://api.together.xyz";

/// Together AI inference client — access to 100+ open-source models.
#[derive(Clone)]
pub struct TogetherClient(OpenAiCompatibleClient);

impl TogetherClient {
    /// Create a new Together AI client.
    ///
    /// Popular models:
    /// - `"meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo"`
    /// - `"mistralai/Mixtral-8x7B-Instruct-v0.1"`
    /// - `"Qwen/Qwen2.5-72B-Instruct-Turbo"`
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self(
            OpenAiCompatibleClient::builder()
                .base_url(TOGETHER_BASE_URL)
                .expect("Valid Together AI base URL")
                .api_key(api_key)
                .default_model(model)
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Valid Together AI config"),
        )
    }

    /// Override the default model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.0.set_default_model(model);
        self
    }
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for TogetherClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        self.0.invoke(input).await
    }

    fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        self.0.stream(input)
    }
}
