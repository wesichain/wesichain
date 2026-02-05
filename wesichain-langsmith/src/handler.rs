use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use regex::Regex;
use serde_json::Value;
use uuid::Uuid;
use wesichain_core::callbacks::{CallbackHandler, RunContext, RunType as CoreRunType};

use crate::{
    ensure_object, sanitize_value, truncate_value, FlushError, FlushStats, LangSmithConfig,
    LangSmithExporter, ProbabilitySampler, RunContextStore, RunEvent, RunType, Sampler,
};

const DEFAULT_MAX_BYTES: usize = 100_000;

pub struct LangSmithCallbackHandler {
    exporter: LangSmithExporter,
    sampler: Arc<dyn Sampler>,
    trace_sampling: DashMap<Uuid, bool>,
    redact_regex: Option<Regex>,
    max_bytes: usize,
    session_name: String,
}

impl LangSmithCallbackHandler {
    pub fn new(config: LangSmithConfig) -> Self {
        let sampler = Arc::new(ProbabilitySampler {
            rate: config.sampling_rate,
        });
        Self::with_sampler(config, sampler)
    }

    pub fn with_sampler(config: LangSmithConfig, sampler: Arc<dyn Sampler>) -> Self {
        let store = Arc::new(RunContextStore::default());
        let exporter = LangSmithExporter::new(config.clone(), store);
        Self {
            exporter,
            sampler,
            trace_sampling: DashMap::new(),
            redact_regex: config.redact_regex.clone(),
            max_bytes: DEFAULT_MAX_BYTES,
            session_name: config.project_name.clone(),
        }
    }

    pub async fn flush(&self, timeout: Duration) -> Result<FlushStats, FlushError> {
        self.exporter.flush(timeout).await
    }

    pub fn dropped_events(&self) -> usize {
        self.exporter.dropped_events()
    }

    fn should_sample(&self, trace_id: Uuid) -> bool {
        if let Some(entry) = self.trace_sampling.get(&trace_id) {
            return *entry;
        }
        let decision = self.sampler.should_sample(trace_id);
        self.trace_sampling.insert(trace_id, decision);
        decision
    }

    fn maybe_clear_trace(&self, ctx: &RunContext) {
        if ctx.parent_run_id.is_none() {
            self.trace_sampling.remove(&ctx.trace_id);
        }
    }

    fn sanitize_object(&self, value: Value) -> Value {
        let sanitized = sanitize_value(value, self.redact_regex.as_ref());
        let truncated = truncate_value(sanitized, self.max_bytes);
        ensure_object(truncated)
    }

    fn sanitize_error(&self, value: Value) -> String {
        let sanitized = sanitize_value(value, self.redact_regex.as_ref());
        let truncated = truncate_value(sanitized, self.max_bytes);
        match truncated {
            Value::String(text) => text,
            other => other.to_string(),
        }
    }

    fn map_run_type(run_type: &CoreRunType) -> RunType {
        match run_type {
            CoreRunType::Chain => RunType::Chain,
            CoreRunType::Tool => RunType::Tool,
            CoreRunType::Llm => RunType::Llm,
            CoreRunType::Agent => RunType::Agent,
            CoreRunType::Graph => RunType::Graph,
            CoreRunType::Retriever | CoreRunType::Runnable => RunType::Chain,
        }
    }
}

#[async_trait::async_trait]
impl CallbackHandler for LangSmithCallbackHandler {
    async fn on_start(&self, ctx: &RunContext, inputs: &Value) {
        if !self.should_sample(ctx.trace_id) {
            return;
        }

        let inputs = self.sanitize_object(inputs.clone());
        let metadata = serde_json::to_value(&ctx.metadata).unwrap_or(Value::Null);
        let metadata = self.sanitize_object(metadata);
        let event = RunEvent::Start {
            run_id: ctx.run_id,
            parent_run_id: ctx.parent_run_id,
            trace_id: ctx.trace_id,
            name: ctx.name.clone(),
            run_type: Self::map_run_type(&ctx.run_type),
            start_time: DateTime::<Utc>::from(ctx.start_time),
            inputs,
            tags: ctx.tags.clone(),
            metadata,
            session_name: self.session_name.clone(),
        };
        self.exporter.enqueue(event).await;
    }

    async fn on_end(&self, ctx: &RunContext, outputs: &Value, duration_ms: u128) {
        if !self.should_sample(ctx.trace_id) {
            self.maybe_clear_trace(ctx);
            return;
        }
        let outputs = self.sanitize_object(outputs.clone());
        let event = RunEvent::Update {
            run_id: ctx.run_id,
            end_time: Some(Utc::now()),
            outputs: Some(outputs),
            error: None,
            duration_ms: Some(duration_ms),
        };
        self.exporter.enqueue(event).await;
        self.maybe_clear_trace(ctx);
    }

    async fn on_error(&self, ctx: &RunContext, error: &Value, duration_ms: u128) {
        if !self.should_sample(ctx.trace_id) {
            self.maybe_clear_trace(ctx);
            return;
        }
        let error = self.sanitize_error(error.clone());
        let event = RunEvent::Update {
            run_id: ctx.run_id,
            end_time: Some(Utc::now()),
            outputs: None,
            error: Some(error),
            duration_ms: Some(duration_ms),
        };
        self.exporter.enqueue(event).await;
        self.maybe_clear_trace(ctx);
    }
}
