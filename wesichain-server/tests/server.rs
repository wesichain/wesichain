//! In-process HTTP server tests using `axum::Router` via `tower::ServiceExt`.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use futures::stream::{self, BoxStream};
use serde_json::{json, Value};
use tower::ServiceExt;
use wesichain_core::{LlmRequest, LlmResponse, Runnable, StreamEvent, WesichainError};
use wesichain_server::chat_router;

// ── Mock LLM ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct MockLlm {
    response: String,
    model_echo: bool,
}

impl MockLlm {
    fn with_response(s: &str) -> Self {
        Self { response: s.to_string(), model_echo: false }
    }
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockLlm {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        Ok(LlmResponse {
            content: self.response.clone(),
            tool_calls: vec![],
            usage: None,
            model: String::new(),
        })
    }

    fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        let text = self.response.clone();
        Box::pin(stream::once(async move {
            Ok(StreamEvent::FinalAnswer(text))
        }))
    }
}

fn build_request(body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn chat_endpoint_returns_200() {
    let app = chat_router(MockLlm::with_response("hello"), "default-model");
    let req = build_request(json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "hi"}]
    }));

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn chat_endpoint_passes_model_to_llm() {
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct RecordingLlm(Arc<Mutex<Vec<String>>>);

    #[async_trait::async_trait]
    impl Runnable<LlmRequest, LlmResponse> for RecordingLlm {
        async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
            self.0.lock().unwrap().push(input.model.clone());
            Ok(LlmResponse {
                content: "ok".to_string(),
                tool_calls: vec![],
                usage: None,
                model: String::new(),
            })
        }
        fn stream(&self, _: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
            Box::pin(stream::empty())
        }
    }

    let models = Arc::new(Mutex::new(Vec::new()));
    let app = chat_router(RecordingLlm(models.clone()), "fallback-model");
    let req = build_request(json!({
        "model": "my-model",
        "messages": [{"role": "user", "content": "hi"}]
    }));

    app.oneshot(req).await.unwrap();
    assert_eq!(models.lock().unwrap()[0], "my-model");
}

#[tokio::test]
async fn chat_endpoint_uses_default_model_when_omitted() {
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct RecordingLlm(Arc<Mutex<Vec<String>>>);

    #[async_trait::async_trait]
    impl Runnable<LlmRequest, LlmResponse> for RecordingLlm {
        async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
            self.0.lock().unwrap().push(input.model.clone());
            Ok(LlmResponse {
                content: "ok".to_string(),
                tool_calls: vec![],
                usage: None,
                model: String::new(),
            })
        }
        fn stream(&self, _: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
            Box::pin(stream::empty())
        }
    }

    let models = Arc::new(Mutex::new(Vec::new()));
    let app = chat_router(RecordingLlm(models.clone()), "default-model");
    // Omit model field
    let req = build_request(json!({"messages": [{"role": "user", "content": "hi"}]}));

    app.oneshot(req).await.unwrap();
    assert_eq!(models.lock().unwrap()[0], "default-model");
}

#[tokio::test]
async fn stream_to_sse_encodes_final_answer() {
    use futures::stream;
    use wesichain_server::sse::stream_to_sse;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;

    let events: BoxStream<'static, Result<StreamEvent, WesichainError>> =
        Box::pin(stream::once(async {
            Ok(StreamEvent::FinalAnswer("finished!".to_string()))
        }));

    let resp = stream_to_sse(events).into_response();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("done"), "SSE body should contain 'done' event");
    assert!(body_str.contains("finished!"), "SSE body should contain answer text");
}
