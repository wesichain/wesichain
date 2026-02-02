use bytes::{Buf, BytesMut};
use futures::{future, stream, stream::StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::error::Category;
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
    parse_ollama_stream(input)
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

fn parse_ndjson_buffer(buffer: &mut BytesMut) -> Result<Vec<StreamEvent>, WesichainError> {
    let mut events = Vec::new();
    let de = serde_json::Deserializer::from_slice(buffer);
    let mut iter = de.into_iter::<OllamaStreamChunk>();
    let mut consumed = 0;

    while let Some(item) = iter.next() {
        match item {
            Ok(chunk) => {
                consumed = iter.byte_offset();
                let event = if chunk.done {
                    StreamEvent::FinalAnswer(chunk.message.content)
                } else {
                    StreamEvent::ContentChunk(chunk.message.content)
                };
                events.push(event);
            }
            Err(err) => {
                if err.classify() == Category::Eof {
                    break;
                }
                let output = String::from_utf8_lossy(buffer).to_string();
                buffer.clear();
                return Err(WesichainError::ParseFailed {
                    output,
                    reason: err.to_string(),
                });
            }
        }
    }

    buffer.advance(consumed);
    Ok(events)
}

fn stream_from_ndjson(
    response: reqwest::Response,
) -> futures::stream::BoxStream<'static, Result<StreamEvent, WesichainError>> {
    let mut buffer = BytesMut::new();
    let terminated = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let terminated_for_take = terminated.clone();
    response
        .bytes_stream()
        .take_while(move |_| {
            future::ready(!terminated_for_take.load(std::sync::atomic::Ordering::SeqCst))
        })
        .flat_map(move |chunk| match chunk {
            Ok(bytes) => {
                buffer.extend_from_slice(&bytes);
                match parse_ndjson_buffer(&mut buffer) {
                    Ok(events) => stream::iter(events.into_iter().map(Ok).collect::<Vec<_>>()),
                    Err(err) => {
                        terminated.store(true, std::sync::atomic::Ordering::SeqCst);
                        stream::iter(vec![Err(err)])
                    }
                }
            }
            Err(err) => {
                terminated.store(true, std::sync::atomic::Ordering::SeqCst);
                stream::iter(vec![Err(WesichainError::LlmProvider(err.to_string()))])
            }
        })
        .boxed()
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
struct OllamaStreamChunk {
    message: OllamaMessage,
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
        input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
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
            stream: true,
        };

        let url = format!("{}/api/chat", self.base_url);
        let fut = self.http.post(url).json(&request).send();
        stream::once(async move {
            fut.await
                .map_err(|err| WesichainError::LlmProvider(err.to_string()))
        })
        .flat_map(|result| match result {
            Ok(resp) => match resp.error_for_status() {
                Ok(resp) => stream_from_ndjson(resp),
                Err(err) => {
                    stream::iter(vec![Err(WesichainError::LlmProvider(err.to_string()))]).boxed()
                }
            },
            Err(err) => stream::iter(vec![Err(err)]).boxed(),
        })
        .boxed()
    }
}
