use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::PineconeStoreError;

#[derive(Clone, Debug)]
pub struct PineconeHttpClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl PineconeHttpClient {
    pub fn new(base_url: String, api_key: String) -> Result<Self, PineconeStoreError> {
        if api_key.trim().is_empty() {
            return Err(PineconeStoreError::Config(
                "api_key cannot be empty".to_string(),
            ));
        }

        reqwest::Url::parse(&base_url)
            .map_err(|err| PineconeStoreError::Config(format!("invalid base_url: {err}")))?;

        Ok(Self {
            http: Client::new(),
            base_url,
            api_key,
        })
    }

    pub async fn post_json(
        &self,
        path: &str,
        payload: &Value,
    ) -> Result<Value, PineconeStoreError> {
        self.post_typed(path, payload).await
    }

    pub async fn post_typed<Req, Resp>(
        &self,
        path: &str,
        payload: &Req,
    ) -> Result<Resp, PineconeStoreError>
    where
        Req: Serialize,
        Resp: DeserializeOwned,
    {
        self.post_typed_with_context(path, payload, None, None)
            .await
    }

    pub async fn post_typed_with_context<Req, Resp>(
        &self,
        path: &str,
        payload: &Req,
        namespace: Option<&str>,
        batch_size: Option<usize>,
    ) -> Result<Resp, PineconeStoreError>
    where
        Req: Serialize,
        Resp: DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        let response = self
            .http
            .post(url)
            .header("Api-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(payload)
            .send()
            .await
            .map_err(|err| PineconeStoreError::Transport(err.to_string()))?;

        let status = response.status();
        if status.is_success() {
            let value = response
                .json::<Resp>()
                .await
                .map_err(|err| PineconeStoreError::Malformed(err.to_string()))?;
            return Ok(value);
        }

        let retry_after_seconds = response
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        let body: Value = response
            .json::<Value>()
            .await
            .unwrap_or_else(|_| Value::String(String::new()));
        let message = body
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| body.get("error").and_then(Value::as_str))
            .unwrap_or("unknown pinecone error")
            .to_string();

        Err(PineconeStoreError::Api {
            status: status.as_u16(),
            message,
            retry_after_seconds,
            namespace: namespace.map(ToOwned::to_owned),
            batch_size,
        })
    }
}
