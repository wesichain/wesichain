use futures::stream::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Deserializer;

use wesichain_core::{Runnable, StreamEvent, WesichainError};

use crate::{LlmRequest, LlmResponse, Message, ToolCall, ToolSpec};

#[derive(Clone)]
pub struct OllamaClient {
    base_url: String,
    model: String,
    http: Client,
}

pub fn ollama_stream_events(input: &[u8]) -> Result<Vec<StreamEvent>, WesichainError> {
    if let Ok(events) = parse_ollama_stream(input) {
        return Ok(events);
    }

    let raw = std::str::from_utf8(input).map_err(|err| WesichainError::ParseFailed {
        output: String::new(),
        reason: err.to_string(),
    })?;
    let wrapped = format!("\"{}\"", raw);
    let decoded: String = serde_json::from_str(&wrapped)?;
    parse_ollama_stream(decoded.as_bytes())
}

fn parse_ollama_stream(input: &[u8]) -> Result<Vec<StreamEvent>, WesichainError> {
    let mut events = Vec::new();
    let stream = Deserializer::from_slice(input).into_iter::<OllamaChatResponse>();
    for item in stream {
        let chunk = item?;
        let event = if chunk.done {
            StreamEvent::FinalAnswer(chunk.message.content)
        } else {
            StreamEvent::ContentChunk(chunk.message.content)
        };
        events.push(event);
    }
    Ok(events)
}

impl OllamaClient {
    pub fn new(base_url: String, model: String) -> Result<Self, WesichainError> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|err| WesichainError::LlmProvider(err.to_string()))?;
        Ok(Self {
            base_url,
            model,
            http,
        })
    }
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(default)]
    tools: Vec<ToolSpec>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
    done: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for OllamaClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        let LlmRequest {
            model,
            messages,
            tools,
        } = input;
        let model = if model.is_empty() {
            self.model.clone()
        } else {
            model
        };
        let request = OllamaChatRequest {
            model,
            messages,
            tools,
            stream: false,
        };

        let url = format!("{}/api/chat", self.base_url);
        let response: OllamaChatResponse = self
            .http
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|err| WesichainError::LlmProvider(err.to_string()))?
            .error_for_status()
            .map_err(|err| WesichainError::LlmProvider(err.to_string()))?
            .json()
            .await
            .map_err(|err| WesichainError::LlmProvider(err.to_string()))?;

        Ok(LlmResponse {
            content: response.message.content,
            tool_calls: response.tool_calls,
        })
    }

    fn stream(
        &self,
        _input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}
