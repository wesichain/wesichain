//! [`CallbackHandler`] implementation that ships events to Langfuse.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use uuid::Uuid;
use wesichain_core::{CallbackHandler, LlmInput, LlmResult, RunContext};

use crate::client::LangfuseClient;
use crate::config::LangfuseConfig;
use crate::types::{
    LangfuseEvent, LangfuseGeneration, LangfuseGenerationUpdate, LangfuseIngestionBatch,
    LangfuseSpan, LangfuseSpanUpdate, LangfuseTrace,
};

// ── Background exporter ───────────────────────────────────────────────────────

struct Exporter {
    client: LangfuseClient,
    rx: mpsc::UnboundedReceiver<LangfuseEvent>,
    batch_size: usize,
    flush_interval: Duration,
}

impl Exporter {
    async fn run(mut self) {
        let mut buffer: Vec<LangfuseEvent> = Vec::new();
        let mut interval = tokio::time::interval(self.flush_interval);

        loop {
            tokio::select! {
                event = self.rx.recv() => {
                    match event {
                        Some(e) => {
                            buffer.push(e);
                            if buffer.len() >= self.batch_size {
                                self.flush(&mut buffer).await;
                            }
                        }
                        None => {
                            if !buffer.is_empty() {
                                self.flush(&mut buffer).await;
                            }
                            return;
                        }
                    }
                }
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        self.flush(&mut buffer).await;
                    }
                }
            }
        }
    }

    async fn flush(&self, buffer: &mut Vec<LangfuseEvent>) {
        if buffer.is_empty() {
            return;
        }
        let batch = LangfuseIngestionBatch { batch: std::mem::take(buffer) };
        if let Err(e) = self.client.ingest(batch).await {
            eprintln!("[wesichain-langfuse] flush error: {e}");
        }
    }
}

// ── Active run state ──────────────────────────────────────────────────────────

struct RunMeta {
    trace_id: String,
    span_id: String,
}

// ── LangfuseCallbackHandler ───────────────────────────────────────────────────

/// A [`CallbackHandler`] that records wesichain run events to Langfuse.
///
/// Internally spawns a background Tokio task that batches events and POSTs
/// them to the Langfuse ingestion API.
pub struct LangfuseCallbackHandler {
    tx: mpsc::UnboundedSender<LangfuseEvent>,
    active_runs: Arc<DashMap<Uuid, RunMeta>>,
    project_name: String,
}

impl LangfuseCallbackHandler {
    pub fn new(config: LangfuseConfig) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let exporter = Exporter {
            client: LangfuseClient::new(&config),
            rx,
            batch_size: config.batch_size,
            flush_interval: Duration::from_secs(config.flush_interval_secs),
        };
        tokio::spawn(exporter.run());
        Self {
            tx,
            active_runs: Arc::new(DashMap::new()),
            project_name: config.project_name,
        }
    }

    fn send(&self, event: LangfuseEvent) {
        let _ = self.tx.send(event);
    }

    fn run_type_str(ctx: &RunContext) -> &'static str {
        use wesichain_core::RunType;
        match ctx.run_type {
            RunType::Chain => "chain",
            RunType::Llm => "llm",
            RunType::Tool => "tool",
            RunType::Graph => "graph",
            RunType::Agent => "agent",
            RunType::Retriever => "retriever",
            RunType::Runnable => "runnable",
        }
    }
}

#[async_trait]
impl CallbackHandler for LangfuseCallbackHandler {
    async fn on_start(&self, ctx: &RunContext, inputs: &Value) {
        let trace_id = Uuid::new_v4().to_string();
        let span_id = Uuid::new_v4().to_string();

        let trace = LangfuseTrace::new(
            trace_id.clone(),
            format!("{}/{}", self.project_name, Self::run_type_str(ctx)),
        );
        self.send(LangfuseEvent::TraceCreate(trace));

        let mut span = LangfuseSpan::new(
            span_id.clone(),
            trace_id.clone(),
            format!("{}:{}", Self::run_type_str(ctx), ctx.name),
        );
        span.input = Some(inputs.clone());
        self.send(LangfuseEvent::SpanCreate(span));

        self.active_runs.insert(ctx.run_id, RunMeta { trace_id, span_id });
    }

    async fn on_end(&self, ctx: &RunContext, outputs: &Value, _duration_ms: u128) {
        if let Some((_, meta)) = self.active_runs.remove(&ctx.run_id) {
            let update = LangfuseSpanUpdate {
                id: meta.span_id,
                trace_id: meta.trace_id,
                end_time: Utc::now(),
                output: Some(outputs.clone()),
                level: Some("DEFAULT".into()),
            };
            self.send(LangfuseEvent::SpanUpdate(update));
        }
    }

    async fn on_error(&self, ctx: &RunContext, error: &Value, _duration_ms: u128) {
        if let Some((_, meta)) = self.active_runs.remove(&ctx.run_id) {
            let update = LangfuseSpanUpdate {
                id: meta.span_id,
                trace_id: meta.trace_id,
                end_time: Utc::now(),
                output: Some(json!({ "error": error })),
                level: Some("ERROR".into()),
            };
            self.send(LangfuseEvent::SpanUpdate(update));
        }
    }

    async fn on_llm_start(&self, ctx: &RunContext, input: &LlmInput) {
        let trace_id = self
            .active_runs
            .get(&ctx.run_id)
            .map(|m| m.trace_id.clone())
            .unwrap_or_else(|| ctx.trace_id.to_string());

        let gen_id = Uuid::new_v4().to_string();
        let mut gen = LangfuseGeneration::new(gen_id, trace_id, "llm", &input.model);
        gen.input = Some(json!({ "prompt": input.prompt }));
        self.send(LangfuseEvent::GenerationCreate(gen));
    }

    async fn on_llm_end(&self, ctx: &RunContext, result: &LlmResult, _duration_ms: u128) {
        let trace_id = self
            .active_runs
            .get(&ctx.run_id)
            .map(|m| m.trace_id.clone())
            .unwrap_or_else(|| ctx.trace_id.to_string());

        let gen_id = Uuid::new_v4().to_string();
        let output = json!({ "generations": result.generations });

        let update = LangfuseGenerationUpdate {
            id: gen_id,
            trace_id,
            end_time: Utc::now(),
            output: Some(output),
            prompt_tokens: result.token_usage.as_ref().map(|u| u.prompt_tokens),
            completion_tokens: result.token_usage.as_ref().map(|u| u.completion_tokens),
            total_tokens: result.token_usage.as_ref().map(|u| u.total_tokens),
        };
        self.send(LangfuseEvent::GenerationUpdate(update));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn make_ctx() -> RunContext {
        RunContext::root(
            wesichain_core::RunType::Chain,
            "test-chain".into(),
            vec![],
            BTreeMap::new(),
        )
    }

    #[tokio::test]
    async fn handler_does_not_panic_on_start_end() {
        let config = LangfuseConfig {
            public_key: "pk-test".into(),
            secret_key: "sk-test".into(),
            host: "http://localhost:9999".into(),
            ..Default::default()
        };
        let handler = LangfuseCallbackHandler::new(config);
        let ctx = make_ctx();
        handler.on_start(&ctx, &json!({"input": "hello"})).await;
        handler.on_end(&ctx, &json!({"output": "world"}), 42).await;
    }

    #[tokio::test]
    async fn handler_error_removes_active_run() {
        let config = LangfuseConfig {
            host: "http://localhost:9999".into(),
            ..Default::default()
        };
        let handler = LangfuseCallbackHandler::new(config);
        let ctx = make_ctx();
        handler.on_start(&ctx, &json!({})).await;
        assert!(handler.active_runs.contains_key(&ctx.run_id));
        handler.on_error(&ctx, &json!({"error": "boom"}), 0).await;
        assert!(!handler.active_runs.contains_key(&ctx.run_id));
    }
}
