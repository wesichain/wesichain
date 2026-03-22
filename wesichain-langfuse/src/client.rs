//! HTTP client for the Langfuse ingestion API.

use reqwest::Client;
use thiserror::Error;

use crate::config::LangfuseConfig;
use crate::types::LangfuseIngestionBatch;

#[derive(Debug, Error)]
pub enum LangfuseError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Langfuse API error {status}: {body}")]
    Api { status: u16, body: String },
}

/// Low-level HTTP client that POSTs batches to `POST /api/public/ingestion`.
#[derive(Clone)]
pub struct LangfuseClient {
    http: Client,
    ingest_url: String,
    public_key: String,
    secret_key: String,
}

impl LangfuseClient {
    pub fn new(config: &LangfuseConfig) -> Self {
        let ingest_url = format!("{}/api/public/ingestion", config.host.trim_end_matches('/'));
        Self {
            http: Client::new(),
            ingest_url,
            public_key: config.public_key.clone(),
            secret_key: config.secret_key.clone(),
        }
    }

    /// Ship a batch of events to Langfuse.
    pub async fn ingest(&self, batch: LangfuseIngestionBatch) -> Result<(), LangfuseError> {
        let resp = self
            .http
            .post(&self.ingest_url)
            .basic_auth(&self.public_key, Some(&self.secret_key))
            .json(&batch)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(LangfuseError::Api { status, body });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LangfuseEvent, LangfuseTrace};

    #[test]
    fn batch_serializes_correctly() {
        let trace = LangfuseTrace::new("trace-1", "test-run");
        let batch = LangfuseIngestionBatch {
            batch: vec![LangfuseEvent::TraceCreate(trace)],
        };
        let json = serde_json::to_string(&batch).unwrap();
        assert!(json.contains("\"type\":\"trace-create\""));
        assert!(json.contains("\"id\":\"trace-1\""));
    }

    #[test]
    fn client_builds_correct_url() {
        let config = LangfuseConfig {
            host: "https://us.cloud.langfuse.com".into(),
            ..Default::default()
        };
        let client = LangfuseClient::new(&config);
        assert_eq!(client.ingest_url, "https://us.cloud.langfuse.com/api/public/ingestion");
    }
}
