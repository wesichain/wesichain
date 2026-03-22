//! OpenTelemetry tracing integration for Wesichain.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use wesichain_otel::{init_otlp, OtelCallbackHandler};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let _provider = init_otlp("my-service")?;
//! let handler = OtelCallbackHandler::new();
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use opentelemetry::{
    global::{self, BoxedSpan},
    trace::{Span, SpanKind, Status, Tracer},
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace::TracerProvider, Resource};
use uuid::Uuid;

use wesichain_core::{CallbackHandler, LlmResult, RunContext, Value};

/// OpenTelemetry callback handler that creates one span per Wesichain run.
///
/// Spans are properly paired: `on_start` opens the span and `on_end` /
/// `on_error` closes it with the correct status and duration.  Previously the
/// handler opened *and* closed the span inside `on_start` (zero-duration
/// orphaned spans) and created a separate unrelated span in `on_end`.
#[derive(Clone)]
pub struct OtelCallbackHandler {
    /// Live spans keyed by run ID.  Removed (and ended) in on_end / on_error.
    active_spans: Arc<DashMap<Uuid, BoxedSpan>>,
}

impl OtelCallbackHandler {
    pub fn new() -> Self {
        Self {
            active_spans: Arc::new(DashMap::new()),
        }
    }
}

impl Default for OtelCallbackHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CallbackHandler for OtelCallbackHandler {
    /// Open a span for the run.  The span remains open until `on_end` or
    /// `on_error` is called.
    async fn on_start(&self, ctx: &RunContext, _inputs: &Value) {
        let tracer = global::tracer("wesichain");
        let mut span = tracer
            .span_builder(ctx.name.clone())
            .with_kind(SpanKind::Internal)
            .start(&tracer);
        span.set_attribute(KeyValue::new("wesichain.run_id", ctx.run_id.to_string()));
        span.set_attribute(KeyValue::new(
            "wesichain.run_type",
            format!("{:?}", ctx.run_type),
        ));
        span.set_attribute(KeyValue::new("wesichain.name", ctx.name.clone()));
        // Store the live span — do NOT call span.end() here.
        self.active_spans.insert(ctx.run_id, span);
    }

    /// Close the span with a success status and the elapsed duration.
    async fn on_end(&self, ctx: &RunContext, _outputs: &Value, duration_ms: u128) {
        if let Some((_, mut span)) = self.active_spans.remove(&ctx.run_id) {
            span.set_attribute(KeyValue::new(
                "wesichain.duration_ms",
                i64::try_from(duration_ms).unwrap_or(i64::MAX),
            ));
            span.set_status(Status::Ok);
            span.end();
        }
    }

    /// Close the span with an error status.
    async fn on_error(&self, ctx: &RunContext, error: &Value, duration_ms: u128) {
        if let Some((_, mut span)) = self.active_spans.remove(&ctx.run_id) {
            span.set_attribute(KeyValue::new(
                "wesichain.duration_ms",
                i64::try_from(duration_ms).unwrap_or(i64::MAX),
            ));
            span.set_status(Status::error(error.to_string()));
            span.end();
        }
    }

    /// Record token usage on the active span without closing it.
    async fn on_llm_end(&self, ctx: &RunContext, result: &LlmResult, _duration_ms: u128) {
        if let Some(usage) = &result.token_usage {
            if let Some(mut span) = self.active_spans.get_mut(&ctx.run_id) {
                span.set_attribute(KeyValue::new(
                    "wesichain.prompt_tokens",
                    i64::from(usage.prompt_tokens),
                ));
                span.set_attribute(KeyValue::new(
                    "wesichain.completion_tokens",
                    i64::from(usage.completion_tokens),
                ));
            }
        }
    }
}

/// Initialize an OTLP tracer provider.
///
/// Reads `OTEL_EXPORTER_OTLP_ENDPOINT` from the environment (default: `http://localhost:4317`).
pub fn init_otlp(service_name: &str) -> Result<TracerProvider, opentelemetry::trace::TraceError> {
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(
            std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string()),
        );

    let span_exporter = exporter.build_span_exporter()?;

    let provider = TracerProvider::builder()
        .with_batch_exporter(span_exporter, runtime::Tokio)
        .with_config(
            opentelemetry_sdk::trace::config().with_resource(Resource::new(vec![KeyValue::new(
                opentelemetry_semantic_conventions::resource::SERVICE_NAME,
                service_name.to_string(),
            )])),
        )
        .build();

    global::set_tracer_provider(provider.clone());
    Ok(provider)
}
