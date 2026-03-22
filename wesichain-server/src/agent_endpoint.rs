//! Agent streaming endpoint: `POST /agent/chat` → SSE stream.

use std::sync::Arc;

use axum::{
    extract::State,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use futures::{stream::BoxStream, StreamExt};
use serde::Deserialize;
use wesichain_core::{StreamEvent, WesichainError};

use crate::sse::stream_to_sse;

#[derive(Debug, Deserialize)]
pub struct AgentChatRequest {
    pub message: String,
}

/// A factory function that turns a message into a streaming agent response.
pub type AgentHandler =
    Arc<dyn Fn(String) -> BoxStream<'static, Result<StreamEvent, WesichainError>> + Send + Sync>;

/// Build a router with `POST /agent/chat` that streams agent events as SSE.
///
/// # Example
/// ```ignore
/// let handler: AgentHandler = Arc::new(move |msg| {
///     Box::pin(my_agent.stream(msg))
/// });
/// let router = agent_router(handler);
/// ```
pub fn agent_router(handler: AgentHandler) -> Router {
    Router::new()
        .route("/agent/chat", post(agent_handler))
        .with_state(handler)
}

async fn agent_handler(
    State(handler): State<AgentHandler>,
    Json(req): Json<AgentChatRequest>,
) -> impl IntoResponse {
    let stream = handler(req.message).boxed();
    stream_to_sse(stream)
}
