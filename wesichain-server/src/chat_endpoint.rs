//! OpenAI-compatible `/v1/chat/completions` endpoint.

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wesichain_core::{LlmRequest, LlmResponse, Message, MessageContent, Role, Runnable};

use crate::sse::stream_to_sse;

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ChatCompletionWireRequest {
    pub model: Option<String>,
    pub messages: Vec<WireMessage>,
    #[serde(default)]
    pub stream: bool,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct WireMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionWireResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<WireChoice>,
}

#[derive(Debug, Serialize)]
pub struct WireChoice {
    pub index: u32,
    pub message: WireAssistantMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct WireAssistantMessage {
    pub role: String,
    pub content: String,
}

// ── Router ────────────────────────────────────────────────────────────────────

/// Build a router with `POST /v1/chat/completions`.
///
/// The `default_model` is used when the request body omits the `model` field.
pub fn chat_router<L>(llm: L, default_model: impl Into<String>) -> Router
where
    L: Runnable<LlmRequest, LlmResponse> + Clone + Send + Sync + 'static,
{
    let state = ChatState {
        llm: Arc::new(llm),
        default_model: default_model.into(),
    };
    Router::new()
        .route("/v1/chat/completions", post(chat_handler::<L>))
        .with_state(Arc::new(state))
}

struct ChatState<L> {
    llm: Arc<L>,
    default_model: String,
}

async fn chat_handler<L>(
    State(state): State<Arc<ChatState<L>>>,
    Json(req): Json<ChatCompletionWireRequest>,
) -> Response
where
    L: Runnable<LlmRequest, LlmResponse> + Clone + Send + Sync + 'static,
{
    let messages: Vec<Message> = req
        .messages
        .into_iter()
        .map(|m| {
            let role = match m.role.as_str() {
                "system" => Role::System,
                "assistant" => Role::Assistant,
                "tool" => Role::Tool,
                _ => Role::User,
            };
            Message {
                role,
                content: MessageContent::Text(m.content),
                tool_call_id: None,
                tool_calls: vec![],
            }
        })
        .collect();

    let llm_req = LlmRequest {
        model: req.model.unwrap_or_else(|| state.default_model.clone()),
        messages,
        tools: vec![],
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        stop_sequences: vec![],
    };

    if req.stream {
        let llm = state.llm.clone();
        // Axum SSE requires 'static — bridge via channel
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        tokio::spawn(async move {
            use futures::StreamExt;
            let mut s = llm.stream(llm_req);
            while let Some(item) = s.next().await {
                if tx.send(item).await.is_err() {
                    break;
                }
            }
        });
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx).boxed();
        stream_to_sse(stream).into_response()
    } else {
        match state.llm.invoke(llm_req).await {
            Ok(resp) => {
                let model = resp_model(&resp, &state.default_model);
                let wire = ChatCompletionWireResponse {
                    id: format!("chatcmpl-{}", simple_id()),
                    object: "chat.completion".to_string(),
                    created: unix_now(),
                    model,
                    choices: vec![WireChoice {
                        index: 0,
                        message: WireAssistantMessage {
                            role: "assistant".to_string(),
                            content: resp.content,
                        },
                        finish_reason: "stop".to_string(),
                    }],
                };
                Json(wire).into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": {"message": e.to_string()}})),
            )
                .into_response(),
        }
    }
}

fn resp_model(resp: &LlmResponse, fallback: &str) -> String {
    // LlmResponse.content is a String; derive the model from usage or fallback
    let _ = resp;
    fallback.to_string()
}

fn simple_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{nanos:x}")
}

fn unix_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
