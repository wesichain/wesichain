use std::fmt;

use crate::{QdrantStoreError, QdrantVectorStore};

#[derive(Default, Clone)]
pub struct QdrantStoreBuilder {
    base_url: Option<String>,
    collection: Option<String>,
    api_key: Option<String>,
}

impl fmt::Debug for QdrantStoreBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let api_key = if self.api_key.is_some() {
            "<redacted>"
        } else {
            "<none>"
        };

        f.debug_struct("QdrantStoreBuilder")
            .field("base_url", &self.base_url)
            .field("collection", &self.collection)
            .field("api_key", &api_key)
            .finish()
    }
}

impl QdrantStoreBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base_url(mut self, value: impl Into<String>) -> Self {
        self.base_url = Some(value.into());
        self
    }

    pub fn collection(mut self, value: impl Into<String>) -> Self {
        self.collection = Some(value.into());
        self
    }

    pub fn api_key(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.api_key = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
        self
    }

    pub fn build(self) -> Result<QdrantVectorStore, QdrantStoreError> {
        let base_url = self.base_url.ok_or(QdrantStoreError::MissingBaseUrl)?;
        if base_url.trim().is_empty() {
            return Err(QdrantStoreError::EmptyBaseUrl);
        }

        let collection = self.collection.ok_or(QdrantStoreError::MissingCollection)?;
        if collection.trim().is_empty() {
            return Err(QdrantStoreError::EmptyCollection);
        }

        if looks_like_qdrant_cloud(&base_url) && self.api_key.is_none() {
            tracing::warn!(
                base_url = %base_url,
                "qdrant cloud URL detected without an API key; requests may fail"
            );
        }

        Ok(QdrantVectorStore {
            client: reqwest::Client::new(),
            base_url,
            collection,
            api_key: self.api_key,
        })
    }
}

fn looks_like_qdrant_cloud(base_url: &str) -> bool {
    const CLOUD_DOMAIN: &[u8] = b"cloud.qdrant.io";

    base_url
        .as_bytes()
        .windows(CLOUD_DOMAIN.len())
        .any(|window| window.eq_ignore_ascii_case(CLOUD_DOMAIN))
}
