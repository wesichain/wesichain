//! Structured types for LLM observability.
//!
//! These types capture LLM-specific inputs and outputs for cost tracking,
//! prompt debugging, and performance analysis.

/// Token consumption for cost tracking and optimization.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// LLM call parameters captured at start time.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LlmInput {
    pub model: String,
    /// Rendered prompt (after template expansion), not the template itself
    pub prompt: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
}

/// LLM call results captured at end time.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LlmResult {
    pub token_usage: Option<TokenUsage>,
    pub model: String,
    pub finish_reason: Option<String>,
    /// Rendered output strings (one per generation)
    pub generations: Vec<String>,
}
