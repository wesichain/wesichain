//! Google Gemini API LLM client

use bytes::BytesMut;
use futures::{
    future,
    stream::{self, BoxStream, StreamExt},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use wesichain_core::{
    LlmRequest, LlmResponse, Message, Role, Runnable, StreamEvent, ToolCall, ToolSpec,
    WesichainError,
};

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";

#[derive(Clone)]
pub struct GoogleClient {
    base_url: String,
    api_key: String,
    model: String,
    http: Client,
}

impl GoogleClient {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let timeout = Duration::from_secs(120);
        let http = Client::builder()
            .timeout(timeout)
            .build()
            .expect("valid reqwest client config");
        Self {
            base_url: GEMINI_BASE_URL.to_string(),
            api_key: api_key.into(),
            model: model.into(),
            http,
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    fn model_name(&self, request_model: &str) -> String {
        let model = if request_model.is_empty() {
            self.model.as_str()
        } else {
            request_model
        };
        model
            .trim()
            .strip_prefix("models/")
            .unwrap_or(model)
            .to_string()
    }

    fn generate_url(&self, request_model: &str) -> String {
        format!(
            "{}/v1beta/models/{}:generateContent",
            self.base_url.trim_end_matches('/'),
            self.model_name(request_model)
        )
    }

    fn stream_url(&self, request_model: &str) -> String {
        format!(
            "{}/v1beta/models/{}:streamGenerateContent",
            self.base_url.trim_end_matches('/'),
            self.model_name(request_model)
        )
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerateContentRequest {
    contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_config: Option<ToolConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Content {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<Part>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function_call: Option<FunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function_response: Option<FunctionResponse>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FunctionCall {
    name: String,
    args: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FunctionResponse {
    name: String,
    response: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiTool {
    function_declarations: Vec<FunctionDeclaration>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FunctionDeclaration {
    name: String,
    description: String,
    parameters_json_schema: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolConfig {
    function_calling_config: FunctionCallingConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FunctionCallingConfig {
    mode: String,
    allowed_function_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateContentResponse {
    candidates: Option<Vec<Candidate>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    content: Option<Content>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleErrorResponse {
    error: GoogleErrorDetail,
}

#[derive(Debug, Deserialize)]
struct GoogleErrorDetail {
    message: String,
}

fn map_tools(tools: &[ToolSpec]) -> Option<Vec<GeminiTool>> {
    if tools.is_empty() {
        return None;
    }
    Some(vec![GeminiTool {
        function_declarations: tools
            .iter()
            .map(|tool| FunctionDeclaration {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters_json_schema: tool.parameters.clone(),
            })
            .collect(),
    }])
}

fn tool_config(tools: &[ToolSpec]) -> Option<ToolConfig> {
    if tools.is_empty() {
        return None;
    }
    Some(ToolConfig {
        function_calling_config: FunctionCallingConfig {
            mode: "AUTO".to_string(),
            allowed_function_names: tools.iter().map(|tool| tool.name.clone()).collect(),
        },
    })
}

fn parse_tool_output(content: &str) -> Value {
    serde_json::from_str::<Value>(content).unwrap_or_else(|_| json!({ "content": content }))
}

fn map_contents(messages: &[Message]) -> Vec<Content> {
    let mut tool_names_by_id: HashMap<String, String> = HashMap::new();
    let mut contents = Vec::new();

    for message in messages {
        match message.role {
            Role::System => {}
            Role::User => {
                contents.push(Content {
                    role: Some("user".to_string()),
                    parts: vec![Part {
                        text: Some(message.content.clone()),
                        function_call: None,
                        function_response: None,
                    }],
                });
            }
            Role::Assistant => {
                let mut parts = Vec::new();
                if !message.content.is_empty() {
                    parts.push(Part {
                        text: Some(message.content.clone()),
                        function_call: None,
                        function_response: None,
                    });
                }

                for call in &message.tool_calls {
                    tool_names_by_id.insert(call.id.clone(), call.name.clone());
                    parts.push(Part {
                        text: None,
                        function_call: Some(FunctionCall {
                            name: call.name.clone(),
                            args: call.args.clone(),
                        }),
                        function_response: None,
                    });
                }

                if !parts.is_empty() {
                    contents.push(Content {
                        role: Some("model".to_string()),
                        parts,
                    });
                }
            }
            Role::Tool => {
                let call_id = message.tool_call_id.clone();
                let name = call_id
                    .as_ref()
                    .and_then(|id| tool_names_by_id.get(id).cloned())
                    .unwrap_or_else(|| "tool".to_string());
                contents.push(Content {
                    role: Some("user".to_string()),
                    parts: vec![Part {
                        text: None,
                        function_call: None,
                        function_response: Some(FunctionResponse {
                            name,
                            response: parse_tool_output(&message.content),
                            id: call_id,
                        }),
                    }],
                });
            }
        }
    }

    contents
}

fn parse_sse_line(line: &str) -> Option<&str> {
    line.trim().strip_prefix("data: ")
}

fn is_blocked_finish_reason(reason: &str) -> bool {
    matches!(reason, "SAFETY" | "RECITATION" | "BLOCKLIST")
}

fn map_candidate_content(
    content: Content,
    accumulated_text: &mut String,
    tool_call_count: &mut usize,
) -> Vec<Result<StreamEvent, WesichainError>> {
    let mut events = Vec::new();
    for part in content.parts {
        if let Some(text) = part.text {
            accumulated_text.push_str(&text);
            events.push(Ok(StreamEvent::ContentChunk(text)));
        }

        if let Some(call) = part.function_call {
            *tool_call_count += 1;
            let id = format!("google_stream_call_{}", tool_call_count);
            let delta = if call.args.is_object() {
                call.args
            } else {
                json!({ "value": call.args })
            };
            events.push(Ok(StreamEvent::ToolCallStart {
                id: id.clone(),
                name: call.name,
            }));
            events.push(Ok(StreamEvent::ToolCallDelta { id, delta }));
        }
    }
    events
}

fn parse_stream_response(
    response: reqwest::Response,
) -> BoxStream<'static, Result<StreamEvent, WesichainError>> {
    let stream = response.bytes_stream();
    let mut buffer = BytesMut::new();
    let mut accumulated_text = String::new();
    let mut tool_call_count = 0usize;
    let terminated = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let terminated_for_take = terminated.clone();

    stream
        .take_while(move |_| {
            future::ready(!terminated_for_take.load(std::sync::atomic::Ordering::SeqCst))
        })
        .flat_map(move |chunk| match chunk {
            Ok(bytes) => {
                buffer.extend_from_slice(&bytes);
                let mut events = Vec::new();

                while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                    let line = buffer.split_to(pos + 1);
                    let line_str = String::from_utf8_lossy(&line);
                    let Some(data) = parse_sse_line(&line_str) else {
                        continue;
                    };

                    if data == "[DONE]" {
                        events.push(Ok(StreamEvent::FinalAnswer(accumulated_text.clone())));
                        terminated.store(true, std::sync::atomic::Ordering::SeqCst);
                        continue;
                    }

                    match serde_json::from_str::<GenerateContentResponse>(data) {
                        Ok(response) => {
                            if let Some(candidate) = response
                                .candidates
                                .and_then(|candidates| candidates.into_iter().next())
                            {
                                if let Some(content) = candidate.content {
                                    events.extend(map_candidate_content(
                                        content,
                                        &mut accumulated_text,
                                        &mut tool_call_count,
                                    ));
                                }
                            }
                        }
                        Err(err) => {
                            terminated.store(true, std::sync::atomic::Ordering::SeqCst);
                            events.push(Err(WesichainError::ParseFailed {
                                output: data.to_string(),
                                reason: err.to_string(),
                            }));
                            break;
                        }
                    }
                }

                stream::iter(events)
            }
            Err(err) => {
                terminated.store(true, std::sync::atomic::Ordering::SeqCst);
                stream::iter(vec![Err(WesichainError::LlmProvider(err.to_string()))])
            }
        })
        .boxed()
}

fn build_request(input: &LlmRequest) -> GenerateContentRequest {
    GenerateContentRequest {
        contents: map_contents(&input.messages),
        system_instruction: system_instruction(&input.messages),
        tools: map_tools(&input.tools),
        tool_config: tool_config(&input.tools),
    }
}

fn system_instruction(messages: &[Message]) -> Option<Content> {
    let parts: Vec<Part> = messages
        .iter()
        .filter(|message| matches!(message.role, Role::System))
        .map(|message| Part {
            text: Some(message.content.clone()),
            function_call: None,
            function_response: None,
        })
        .collect();

    if parts.is_empty() {
        None
    } else {
        Some(Content { role: None, parts })
    }
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for GoogleClient {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        let request = build_request(&input);

        let response = self
            .http
            .post(self.generate_url(&input.model))
            .query(&[("key", self.api_key.as_str())])
            .json(&request)
            .send()
            .await
            .map_err(|err| WesichainError::LlmProvider(err.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<GoogleErrorResponse>(&body)
                .map(|e| e.error.message)
                .unwrap_or_else(|_| format!("HTTP {}: {}", status, body));
            return Err(WesichainError::LlmProvider(message));
        }

        let response = response
            .json::<GenerateContentResponse>()
            .await
            .map_err(|err| WesichainError::LlmProvider(err.to_string()))?;

        let candidate = response
            .candidates
            .and_then(|candidates| candidates.into_iter().next())
            .ok_or_else(|| WesichainError::LlmProvider("No candidates in response".to_string()))?;

        let finish_reason = candidate.finish_reason.clone();

        let content = candidate.content.unwrap_or(Content {
            role: None,
            parts: vec![],
        });

        let mut tool_calls = Vec::new();
        let text = content
            .parts
            .into_iter()
            .filter_map(|part| {
                if let Some(call) = part.function_call {
                    let args = if call.args.is_object() {
                        call.args
                    } else {
                        json!({ "value": call.args })
                    };
                    let id = format!("google_call_{}", tool_calls.len() + 1);
                    tool_calls.push(ToolCall {
                        id,
                        name: call.name,
                        args,
                    });
                }
                part.text
            })
            .collect::<Vec<_>>()
            .join("");

        if text.is_empty()
            && tool_calls.is_empty()
            && finish_reason
                .as_deref()
                .map(is_blocked_finish_reason)
                .unwrap_or(false)
        {
            let reason = finish_reason.unwrap_or_else(|| "UNKNOWN".to_string());
            return Err(WesichainError::LlmProvider(format!(
                "Generation blocked: {}",
                reason
            )));
        }

        Ok(LlmResponse {
            content: text,
            tool_calls,
        })
    }

    fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        let request = build_request(&input);
        let client = self.clone();

        stream::once(async move {
            client
                .http
                .post(client.stream_url(&input.model))
                .query(&[("key", client.api_key.as_str())])
                .json(&request)
                .send()
                .await
                .map_err(|err| WesichainError::LlmProvider(err.to_string()))
        })
        .flat_map(|result| match result {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    parse_stream_response(response)
                } else {
                    stream::once(async move {
                        let body = response.text().await.unwrap_or_default();
                        let message = serde_json::from_str::<GoogleErrorResponse>(&body)
                            .map(|e| e.error.message)
                            .unwrap_or_else(|_| format!("HTTP {}: {}", status, body));
                        Err(WesichainError::LlmProvider(message))
                    })
                    .boxed()
                }
            }
            Err(err) => stream::iter(vec![Err(err)]).boxed(),
        })
        .boxed()
    }
}
