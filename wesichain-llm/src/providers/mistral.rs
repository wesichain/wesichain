//! Mistral AI client.
//!
//! Wraps [`OpenAiCompatibleClient`] with the Mistral base URL.
//! Mistral is fully OpenAI-compatible so no special logic is required.
//!
//! # Example
//! ```ignore
//! let client = MistralClient::new(
//!     env::var("MISTRAL_API_KEY").unwrap(),
//!     "mistral-large-latest",
//! );
//! ```

use std::time::Duration;

use futures::stream::BoxStream;
use wesichain_core::{Runnable, StreamEvent, WesichainError};

use crate::openai_compatible::OpenAiCompatibleClient;
use crate::{LlmRequest, LlmResponse};

const MISTRAL_BASE_URL: &str = "https://api.mistral.ai";

/// Mistral AI client.
#[derive(Clone)]
pub struct MistralClient(OpenAiCompatibleClient);

impl MistralClient {
    /// Create a new Mistral client.
    ///
    /// - `api_key`: Mistral API key
    /// - `model`: model identifier e.g. `"mistral-large-latest"`
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self(
            OpenAiCompatibleClient::builder()
                .base_url(MISTRAL_BASE_URL)
                .expect("Valid Mistral base URL")
                .api_key(api_key)
                .default_model(model)
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Valid Mistral config"),
        )
    }

    /// Override the default model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.0.set_default_model(model);
        self
    }
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for MistralClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        self.0.invoke(input).await
    }

    fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        self.0.stream(input)
    }
}

impl wesichain_core::ToolCallingLlm for MistralClient {}
