//! Groq LLM client.
//!
//! Groq is fully OpenAI-compatible.  Wraps [`OpenAiCompatibleClient`] with the
//! Groq base URL and a short default timeout (Groq is extremely fast).
//!
//! # Example
//! ```ignore
//! use wesichain_llm::providers::groq::GroqClient;
//!
//! let llm = GroqClient::new(
//!     std::env::var("GROQ_API_KEY").unwrap(),
//!     "llama-3.3-70b-versatile",
//! );
//! ```

use std::time::Duration;

use futures::stream::BoxStream;
use wesichain_core::{Runnable, StreamEvent, WesichainError};

use crate::openai_compatible::OpenAiCompatibleClient;
use crate::{LlmRequest, LlmResponse};

const GROQ_BASE_URL: &str = "https://api.groq.com/openai";

/// Groq inference client — fastest open-model inference available.
#[derive(Clone)]
pub struct GroqClient(OpenAiCompatibleClient);

impl GroqClient {
    /// Create a new Groq client.
    ///
    /// Popular models: `"llama-3.3-70b-versatile"`, `"llama-3.1-8b-instant"`,
    /// `"mixtral-8x7b-32768"`, `"gemma2-9b-it"`.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self(
            OpenAiCompatibleClient::builder()
                .base_url(GROQ_BASE_URL)
                .expect("Valid Groq base URL")
                .api_key(api_key)
                .default_model(model)
                .timeout(Duration::from_secs(60))
                .build()
                .expect("Valid Groq config"),
        )
    }

    /// Override the default model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.0.set_default_model(model);
        self
    }
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for GroqClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        self.0.invoke(input).await
    }

    fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        self.0.stream(input)
    }
}
