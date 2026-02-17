use std::fmt;

use crate::{WeaviateStoreError, WeaviateVectorStore};

#[derive(Default, Clone)]
pub struct WeaviateStoreBuilder {
    base_url: Option<String>,
    class_name: Option<String>,
    api_key: Option<String>,
    auto_create_class: bool,
}

impl fmt::Debug for WeaviateStoreBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let api_key = if self.api_key.is_some() {
            "<redacted>"
        } else {
            "<none>"
        };

        f.debug_struct("WeaviateStoreBuilder")
            .field("base_url", &self.base_url)
            .field("class_name", &self.class_name)
            .field("api_key", &api_key)
            .field("auto_create_class", &self.auto_create_class)
            .finish()
    }
}

impl WeaviateStoreBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base_url(mut self, value: impl Into<String>) -> Self {
        self.base_url = Some(value.into());
        self
    }

    pub fn class_name(mut self, value: impl Into<String>) -> Self {
        self.class_name = Some(value.into());
        self
    }

    pub fn api_key(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        let trimmed = value.trim();
        self.api_key = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        self
    }

    pub fn auto_create_class(mut self, value: bool) -> Self {
        self.auto_create_class = value;
        self
    }

    pub fn build(self) -> Result<WeaviateVectorStore, WeaviateStoreError> {
        let base_url = self
            .base_url
            .ok_or(WeaviateStoreError::MissingBaseUrl)?
            .trim()
            .to_string();
        if base_url.is_empty() {
            return Err(WeaviateStoreError::EmptyBaseUrl);
        }

        let class_name = self
            .class_name
            .ok_or(WeaviateStoreError::MissingClassName)?
            .trim()
            .to_string();
        if class_name.is_empty() {
            return Err(WeaviateStoreError::EmptyClassName);
        }

        validate_class_name(&class_name)?;

        if looks_like_weaviate_cloud(&base_url) && self.api_key.is_none() {
            tracing::warn!(
                base_url = %base_url,
                "weaviate cloud URL detected without an API key; requests may fail"
            );
        }

        Ok(WeaviateVectorStore {
            client: reqwest::Client::new(),
            base_url,
            class_name,
            api_key: self.api_key,
            auto_create_class: self.auto_create_class,
        })
    }
}

fn validate_class_name(class_name: &str) -> Result<(), WeaviateStoreError> {
    let mut chars = class_name.chars();
    let Some(first) = chars.next() else {
        return Err(WeaviateStoreError::EmptyClassName);
    };

    if !first.is_ascii_alphabetic() {
        return Err(WeaviateStoreError::InvalidClassName {
            class_name: class_name.to_string(),
            reason: "must start with an ASCII letter".to_string(),
        });
    }

    if let Some(invalid) = class_name
        .chars()
        .find(|ch| !ch.is_ascii_alphanumeric() && *ch != '_')
    {
        return Err(WeaviateStoreError::InvalidClassName {
            class_name: class_name.to_string(),
            reason: format!("contains invalid character '{invalid}'"),
        });
    }

    Ok(())
}

fn looks_like_weaviate_cloud(base_url: &str) -> bool {
    let without_scheme = base_url
        .split_once("://")
        .map_or(base_url, |(_, remainder)| remainder);
    let authority = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let host_port = authority.rsplit('@').next().unwrap_or_default();

    let host = if host_port.starts_with('[') {
        host_port
            .strip_prefix('[')
            .and_then(|rest| rest.split_once(']'))
            .map_or(host_port, |(ipv6, _)| ipv6)
    } else {
        host_port.split(':').next().unwrap_or_default()
    };

    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "cloud.weaviate.io" || host.ends_with(".cloud.weaviate.io")
}

#[cfg(test)]
mod tests {
    use super::looks_like_weaviate_cloud;

    #[test]
    fn cloud_detection_checks_hostname() {
        assert!(looks_like_weaviate_cloud(
            "https://foo.cloud.weaviate.io/v1"
        ));
        assert!(looks_like_weaviate_cloud("https://cloud.weaviate.io"));
    }

    #[test]
    fn cloud_detection_ignores_path_and_query_matches() {
        assert!(!looks_like_weaviate_cloud(
            "https://localhost:8080/cloud.weaviate.io/v1"
        ));
        assert!(!looks_like_weaviate_cloud(
            "https://localhost:8080/v1?target=cloud.weaviate.io"
        ));
    }
}
