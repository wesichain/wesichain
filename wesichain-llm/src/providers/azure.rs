//! Azure OpenAI client.
//!
//! Wraps [`OpenAiCompatibleClient`] with the Azure endpoint URL pattern.
//!
//! # Example
//! ```ignore
//! let client = AzureOpenAiClient::new(
//!     "my-resource",
//!     "my-gpt4-deployment",
//!     env::var("AZURE_OPENAI_KEY").unwrap(),
//! );
//! ```

use std::time::Duration;

use futures::stream::BoxStream;
use secrecy::{ExposeSecret, Secret};
use wesichain_core::{Runnable, StreamEvent, WesichainError};

use crate::openai_compatible::OpenAiCompatibleClient;
use crate::{LlmRequest, LlmResponse};

const API_VERSION: &str = "2024-02-15-preview";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Azure OpenAI client — drop-in replacement for [`OpenAiCompatibleClient`]
/// targeting an Azure-hosted deployment.
#[derive(Clone)]
pub struct AzureOpenAiClient {
    inner: OpenAiCompatibleClient,
    resource: String,
    deployment: String,
    api_key: Secret<String>,
    timeout: Duration,
}

impl AzureOpenAiClient {
    /// Create a new Azure OpenAI client.
    ///
    /// - `resource`: Azure resource name (the subdomain of `.openai.azure.com`)
    /// - `deployment`: deployment name configured in Azure AI Studio
    /// - `api_key`: Azure OpenAI API key
    pub fn new(
        resource: impl AsRef<str>,
        deployment: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, WesichainError> {
        let resource = resource.as_ref().to_string();
        let deployment = deployment.into();
        let api_key = Secret::new(api_key.into());
        let timeout = DEFAULT_TIMEOUT;

        let inner = build_inner(&resource, &deployment, api_key.expose_secret(), timeout)?;
        Ok(Self { inner, resource, deployment, api_key, timeout })
    }

    /// Override the HTTP timeout (default: 120 s).
    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, WesichainError> {
        self.inner = build_inner(&self.resource, &self.deployment, self.api_key.expose_secret(), timeout)?;
        self.timeout = timeout;
        Ok(self)
    }

    pub fn deployment(&self) -> &str {
        &self.deployment
    }
}

fn build_inner(
    resource: &str,
    deployment: &str,
    api_key: &str,
    timeout: Duration,
) -> Result<OpenAiCompatibleClient, WesichainError> {
    let base_url = format!(
        "https://{resource}.openai.azure.com/openai/deployments/{deployment}/chat/completions?api-version={API_VERSION}",
    );
    OpenAiCompatibleClient::builder()
        .base_url(&base_url)?
        .api_key(api_key)
        .default_model(deployment)
        .timeout(timeout)
        .build()
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for AzureOpenAiClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        self.inner.invoke(input).await
    }

    fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        self.inner.stream(input)
    }
}

impl wesichain_core::ToolCallingLlm for AzureOpenAiClient {}
