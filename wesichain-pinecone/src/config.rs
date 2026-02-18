use crate::client::PineconeHttpClient;
use crate::store::PineconeVectorStore;
use crate::PineconeStoreError;
use std::sync::Arc;
use wesichain_core::Embedding;

pub struct PineconeStoreBuilder {
    embedder: Arc<dyn Embedding>,
    base_url: Option<String>,
    api_key: Option<String>,
    namespace: Option<String>,
    text_key: String,
    index_name: Option<String>,
    validate_dimension: bool,
    max_batch_size: usize,
}

impl PineconeStoreBuilder {
    pub fn new<E: Embedding + Send + Sync + 'static>(embedder: E) -> Self {
        Self {
            embedder: Arc::new(embedder),
            base_url: None,
            api_key: None,
            namespace: None,
            text_key: "text".to_string(),
            index_name: None,
            validate_dimension: false,
            max_batch_size: 1000,
        }
    }

    pub fn base_url(mut self, value: impl Into<String>) -> Self {
        self.base_url = Some(value.into());
        self
    }

    pub fn api_key(mut self, value: impl Into<String>) -> Self {
        self.api_key = Some(value.into());
        self
    }

    pub fn namespace(mut self, value: impl Into<String>) -> Self {
        self.namespace = Some(value.into());
        self
    }

    pub fn text_key(mut self, value: impl Into<String>) -> Self {
        self.text_key = value.into();
        self
    }

    pub fn index_name(mut self, value: impl Into<String>) -> Self {
        self.index_name = Some(value.into());
        self
    }

    pub fn validate_dimension(mut self, value: bool) -> Self {
        self.validate_dimension = value;
        self
    }

    pub fn max_batch_size(mut self, value: usize) -> Self {
        self.max_batch_size = value;
        self
    }

    pub fn base_url_from_env(mut self, var_name: &str) -> Self {
        if let Ok(value) = std::env::var(var_name) {
            self.base_url = Some(value);
        }
        self
    }

    pub fn api_key_from_env(mut self, var_name: &str) -> Self {
        if let Ok(value) = std::env::var(var_name) {
            self.api_key = Some(value);
        }
        self
    }

    pub async fn build(self) -> Result<PineconeVectorStore, PineconeStoreError> {
        let base_url = self
            .base_url
            .ok_or_else(|| PineconeStoreError::Config("base_url is required".to_string()))?;
        let api_key = self
            .api_key
            .ok_or_else(|| PineconeStoreError::Config("api_key is required".to_string()))?;

        let client = PineconeHttpClient::new(base_url, api_key)?;
        if self.max_batch_size == 0 {
            return Err(PineconeStoreError::Config(
                "max_batch_size must be greater than 0".to_string(),
            ));
        }

        let store = PineconeVectorStore::new(
            self.embedder,
            client,
            self.namespace,
            self.text_key,
            self.index_name,
            self.validate_dimension,
            self.max_batch_size,
        );
        store.validate_dimension_on_init().await;
        Ok(store)
    }
}
