use std::time::Duration;

use reqwest::{header::HeaderMap, Client, Method, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use thiserror::Error;
use tokio::time::sleep;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum LangSmithError {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("http error: {status}")]
    Http { status: StatusCode, body: String },
}

#[derive(Clone)]
pub struct LangSmithClient {
    client: Client,
    api_url: String,
    api_key: SecretString,
}

impl LangSmithClient {
    pub fn new(api_url: String, api_key: SecretString) -> Self {
        Self {
            client: Client::new(),
            api_url,
            api_key,
        }
    }

    pub async fn create_run(&self, run_id: Uuid, payload: &Value) -> Result<(), LangSmithError> {
        let url = format!("{}/runs", self.api_url.trim_end_matches('/'));
        self.send_with_retry(Method::POST, &url, Some(run_id.to_string()), payload, false)
            .await
    }

    pub async fn update_run(&self, run_id: Uuid, payload: &Value) -> Result<(), LangSmithError> {
        let url = format!("{}/runs/{}", self.api_url.trim_end_matches('/'), run_id);
        self.send_with_retry(Method::PATCH, &url, None, payload, true)
            .await
    }

    async fn send_with_retry(
        &self,
        method: Method,
        url: &str,
        idempotency_key: Option<String>,
        payload: &Value,
        allow_not_found: bool,
    ) -> Result<(), LangSmithError> {
        let mut attempt = 0;
        let mut backoff = Duration::from_millis(200);

        loop {
            attempt += 1;
            let mut request = self
                .client
                .request(method.clone(), url)
                .header("x-api-key", self.api_key.expose_secret())
                .json(payload);
            if let Some(key) = &idempotency_key {
                request = request.header("x-idempotency-key", key);
            }

            match request.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(());
                    }
                    if allow_not_found && response.status() == StatusCode::NOT_FOUND {
                        return Ok(());
                    }
                    if should_retry(response.status()) && attempt < 3 {
                        backoff = next_delay(response.status(), response.headers(), backoff);
                        sleep(backoff).await;
                        continue;
                    }
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    return Err(LangSmithError::Http { status, body });
                }
                Err(err) => {
                    if (err.is_timeout() || err.is_connect()) && attempt < 3 {
                        sleep(backoff).await;
                        backoff = backoff.saturating_mul(2);
                        continue;
                    }
                    return Err(LangSmithError::Request(err));
                }
            }
        }
    }
}

fn should_retry(status: StatusCode) -> bool {
    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
}

fn next_delay(status: StatusCode, headers: &HeaderMap, backoff: Duration) -> Duration {
    if status == StatusCode::TOO_MANY_REQUESTS {
        if let Some(value) = headers.get("Retry-After").and_then(|v| v.to_str().ok()) {
            if let Ok(seconds) = value.parse::<u64>() {
                return Duration::from_secs(seconds);
            }
        }
    }
    backoff.saturating_mul(2)
}
