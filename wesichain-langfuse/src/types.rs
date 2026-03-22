//! Langfuse API payload types.
//!
//! See <https://api.reference.langfuse.com> for the full schema.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Core event wrapper ────────────────────────────────────────────────────────

/// Top-level envelope sent to `POST /api/public/ingestion`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LangfuseIngestionBatch {
    pub batch: Vec<LangfuseEvent>,
}

/// A single ingestion event (discriminated by `type`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LangfuseEvent {
    TraceCreate(LangfuseTrace),
    SpanCreate(LangfuseSpan),
    SpanUpdate(LangfuseSpanUpdate),
    GenerationCreate(LangfuseGeneration),
    GenerationUpdate(LangfuseGenerationUpdate),
}

// ── Trace ─────────────────────────────────────────────────────────────────────

/// A Langfuse trace — the top-level container for a single agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LangfuseTrace {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    pub timestamp: DateTime<Utc>,
}

impl LangfuseTrace {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            user_id: None,
            session_id: None,
            metadata: None,
            timestamp: Utc::now(),
        }
    }
}

// ── Span ──────────────────────────────────────────────────────────────────────

/// A Langfuse span — represents a single step within a trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LangfuseSpan {
    pub id: String,
    pub trace_id: String,
    pub name: String,
    pub start_time: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_observation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
}

impl LangfuseSpan {
    pub fn new(
        id: impl Into<String>,
        trace_id: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            trace_id: trace_id.into(),
            name: name.into(),
            start_time: Utc::now(),
            end_time: None,
            parent_observation_id: None,
            metadata: None,
            input: None,
            output: None,
        }
    }
}

/// Patch to close a span with an end time and optional output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LangfuseSpanUpdate {
    pub id: String,
    pub trace_id: String,
    pub end_time: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
}

// ── Generation ────────────────────────────────────────────────────────────────

/// A Langfuse generation — an LLM call with model, prompt, completion, and usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LangfuseGeneration {
    pub id: String,
    pub trace_id: String,
    pub name: String,
    pub model: String,
    pub start_time: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_observation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

impl LangfuseGeneration {
    pub fn new(
        id: impl Into<String>,
        trace_id: impl Into<String>,
        name: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            trace_id: trace_id.into(),
            name: name.into(),
            model: model.into(),
            start_time: Utc::now(),
            end_time: None,
            parent_observation_id: None,
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            input: None,
            output: None,
            metadata: None,
        }
    }
}

/// Patch to close a generation with end time, usage, and output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LangfuseGenerationUpdate {
    pub id: String,
    pub trace_id: String,
    pub end_time: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
}
