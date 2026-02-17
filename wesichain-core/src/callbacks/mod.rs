use std::collections::BTreeMap;
use std::time::{Instant, SystemTime};

use async_trait::async_trait;
use serde::Serialize;
use uuid::Uuid;

use crate::Value;

mod wrappers;
mod llm;

pub use llm::{LlmInput, LlmResult, TokenUsage};

// pub use wrappers::TracedRunnable;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunType {
    Chain,
    Llm,
    Tool,
    Graph,
    Agent,
    Retriever,
    Runnable,
}

#[derive(Clone, Debug)]
pub struct RunContext {
    pub run_id: Uuid,
    pub parent_run_id: Option<Uuid>,
    pub trace_id: Uuid,
    pub run_type: RunType,
    pub name: String,
    pub start_time: SystemTime,
    pub start_instant: Instant,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, Value>,
}

impl RunContext {
    pub fn root(
        run_type: RunType,
        name: String,
        tags: Vec<String>,
        metadata: BTreeMap<String, Value>,
    ) -> Self {
        let run_id = Uuid::new_v4();
        Self {
            run_id,
            parent_run_id: None,
            trace_id: run_id,
            run_type,
            name,
            start_time: SystemTime::now(),
            start_instant: Instant::now(),
            tags,
            metadata,
        }
    }

    pub fn child(&self, run_type: RunType, name: String) -> Self {
        let run_id = Uuid::new_v4();
        Self {
            run_id,
            parent_run_id: Some(self.run_id),
            trace_id: self.trace_id,
            run_type,
            name,
            start_time: SystemTime::now(),
            start_instant: Instant::now(),
            tags: self.tags.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RunConfig {
    pub callbacks: Option<CallbackManager>,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, Value>,
    pub name_override: Option<String>,
}

#[async_trait]
pub trait CallbackHandler: Send + Sync {
    async fn on_start(&self, ctx: &RunContext, inputs: &Value);
    async fn on_end(&self, ctx: &RunContext, outputs: &Value, duration_ms: u128);
    async fn on_error(&self, ctx: &RunContext, error: &Value, duration_ms: u128);
    async fn on_stream_chunk(&self, _ctx: &RunContext, _chunk: &Value) {}
}

#[derive(Clone, Default)]
pub struct CallbackManager {
    handlers: Vec<std::sync::Arc<dyn CallbackHandler>>,
}

impl std::fmt::Debug for CallbackManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallbackManager")
            .field("handlers", &self.handlers.len())
            .finish()
    }
}

impl CallbackManager {
    pub fn new(handlers: Vec<std::sync::Arc<dyn CallbackHandler>>) -> Self {
        Self { handlers }
    }

    pub fn noop() -> Self {
        Self { handlers: vec![] }
    }

    pub fn is_noop(&self) -> bool {
        self.handlers.is_empty()
    }

    pub async fn on_start(&self, ctx: &RunContext, inputs: &Value) {
        for handler in &self.handlers {
            handler.on_start(ctx, inputs).await;
        }
    }

    pub async fn on_end(&self, ctx: &RunContext, outputs: &Value, duration_ms: u128) {
        for handler in &self.handlers {
            handler.on_end(ctx, outputs, duration_ms).await;
        }
    }

    pub async fn on_error(&self, ctx: &RunContext, error: &Value, duration_ms: u128) {
        for handler in &self.handlers {
            handler.on_error(ctx, error, duration_ms).await;
        }
    }

    pub async fn on_stream_chunk(&self, ctx: &RunContext, chunk: &Value) {
        for handler in &self.handlers {
            handler.on_stream_chunk(ctx, chunk).await;
        }
    }
}

pub trait ToTraceInput {
    fn to_trace_input(&self) -> Value;
}

pub trait ToTraceOutput {
    fn to_trace_output(&self) -> Value;
}

impl<T> ToTraceInput for T
where
    T: Serialize,
{
    fn to_trace_input(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

impl<T> ToTraceOutput for T
where
    T: Serialize,
{
    fn to_trace_output(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

pub fn ensure_object(value: Value) -> Value {
    match value {
        Value::Object(_) => value,
        other => Value::Object(serde_json::Map::from_iter([("value".to_string(), other)])),
    }
}
