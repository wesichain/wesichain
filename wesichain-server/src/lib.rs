//! HTTP server integration for wesichain.
//!
//! Provides a one-liner server builder that exposes LLMs and agents over HTTP.
//!
//! # Quick start
//!
//! ```ignore
//! use wesichain_server::WesichainServer;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     WesichainServer::new()
//!         .with_chat(my_llm, "claude-3-5-sonnet-20241022")
//!         .with_auth_token("my-secret-token")
//!         .serve("0.0.0.0:3000".parse()?)
//!         .await
//! }
//! ```

pub mod agent_endpoint;
pub mod chat_endpoint;
pub mod sse;

pub use agent_endpoint::{agent_router, AgentHandler};
pub use chat_endpoint::chat_router;
pub use sse::stream_to_sse;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::Response,
};
use tower_http::limit::RequestBodyLimitLayer;
use wesichain_core::{LlmRequest, LlmResponse, Runnable, WesichainError};

/// Default maximum request body size (4 MiB).
const DEFAULT_BODY_LIMIT: usize = 4 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Bearer auth middleware
// ---------------------------------------------------------------------------

/// Axum middleware that enforces a static Bearer token.
///
/// Responds with `401 Unauthorized` when the `Authorization` header is absent
/// or does not match `Bearer <token>`.
async fn bearer_auth_middleware(
    State(expected): State<Arc<String>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let provided = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided == format!("Bearer {expected}") {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

// ---------------------------------------------------------------------------
// Server builder
// ---------------------------------------------------------------------------

/// One-liner builder for a wesichain HTTP server.
pub struct WesichainServer {
    router: Router,
    auth_token: Option<String>,
    body_limit: usize,
}

impl Default for WesichainServer {
    fn default() -> Self {
        Self::new()
    }
}

impl WesichainServer {
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            auth_token: None,
            body_limit: DEFAULT_BODY_LIMIT,
        }
    }

    /// Require a static Bearer token on every request.
    ///
    /// Requests without `Authorization: Bearer <token>` will receive `401`.
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Override the maximum request body size (default: 4 MiB).
    pub fn with_body_limit(mut self, bytes: usize) -> Self {
        self.body_limit = bytes;
        self
    }

    /// Mount `POST /v1/chat/completions` backed by the given LLM.
    pub fn with_chat<L>(mut self, llm: L, default_model: impl Into<String>) -> Self
    where
        L: Runnable<LlmRequest, LlmResponse> + Clone + Send + Sync + 'static,
    {
        self.router = self.router.merge(chat_router(llm, default_model));
        self
    }

    /// Mount `POST /agent/chat` that streams agent events as SSE.
    pub fn with_agent_stream(mut self, handler: AgentHandler) -> Self {
        self.router = self.router.merge(agent_router(handler));
        self
    }

    /// Merge any additional Axum router.
    pub fn with_router(mut self, router: Router) -> Self {
        self.router = self.router.merge(router);
        self
    }

    /// Build the finalised Axum [`Router`] with all middleware applied.
    ///
    /// Middleware stack (outermost first):
    /// 1. `RequestBodyLimitLayer` — rejects bodies larger than `body_limit`
    /// 2. Bearer token auth layer (optional) — 401 if token is missing/wrong
    pub fn build(self) -> Router {
        let mut router = self.router;

        // Bearer token auth (innermost — applied before body limit to reject
        // unauthenticated requests before reading the body).
        if let Some(token) = self.auth_token {
            let token_state = Arc::new(token);
            router = router.route_layer(
                middleware::from_fn_with_state(token_state, bearer_auth_middleware),
            );
        }

        // Body size cap (outermost).
        router.layer(RequestBodyLimitLayer::new(self.body_limit))
    }

    /// Start the server and block until it is stopped.
    pub async fn serve(self, addr: SocketAddr) -> Result<(), WesichainError> {
        let app = self.build();
        let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
            WesichainError::InvalidConfig(format!("Failed to bind {addr}: {e}"))
        })?;

        axum::serve(listener, app)
            .await
            .map_err(|e| WesichainError::Custom(format!("Server error: {e}")))
    }
}
