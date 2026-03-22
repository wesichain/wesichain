//! Per-session token cost tracking.
//!
//! [`PriceTable`] maps model names to their USD cost per 1 000 tokens.
//! [`cost_for_response`] computes the cost of a single [`LlmResponse`].
//! [`SessionCostSummary`] accumulates costs over multiple turns.
//!
//! # Example
//! ```
//! use wesichain_session::cost::{cost_for_response, SessionCostSummary};
//! use wesichain_core::{LlmResponse, TokenUsage};
//!
//! let response = LlmResponse {
//!     model: "claude-3-5-sonnet-20241022".to_string(),
//!     content: "Hello".to_string(),
//!     tool_calls: vec![],
//!     usage: Some(TokenUsage { prompt_tokens: 100, completion_tokens: 50, total_tokens: 150 }),
//! };
//!
//! let cost = cost_for_response(&response);
//! let mut summary = SessionCostSummary::default();
//! summary.add(&response);
//! assert!(summary.total_cost_usd > 0.0);
//! ```

use wesichain_core::{LlmResponse, TokenUsage};

// USD per 1 000 tokens: (input_cost, output_cost)
type PricePair = (f64, f64);

/// Look up the price per 1 000 tokens for a model.
///
/// Returns `(input_cost_usd_per_1k, output_cost_usd_per_1k)`.
/// Unknown models return `(0.0, 0.0)`.
pub fn price_for_model(model: &str) -> PricePair {
    let m = model.to_lowercase();
    let m = m.as_str();

    // ── Anthropic ────────────────────────────────────────────────────────────
    if m.contains("claude-opus-4") {
        return (0.015, 0.075);
    }
    if m.contains("claude-sonnet-4") || m.contains("claude-3-7-sonnet") || m.contains("claude-3.7-sonnet") {
        return (0.003, 0.015);
    }
    if m.contains("claude-3-5-sonnet") || m.contains("claude-3.5-sonnet") {
        return (0.003, 0.015);
    }
    if m.contains("claude-3-5-haiku") || m.contains("claude-3.5-haiku") {
        return (0.0008, 0.004);
    }
    if m.contains("claude-3-opus") {
        return (0.015, 0.075);
    }
    if m.contains("claude-3-sonnet") {
        return (0.003, 0.015);
    }
    if m.contains("claude-3-haiku") {
        return (0.00025, 0.00125);
    }
    if m.contains("claude-2") {
        return (0.008, 0.024);
    }

    // ── OpenAI ───────────────────────────────────────────────────────────────
    if m.contains("gpt-4o-mini") {
        return (0.00015, 0.0006);
    }
    if m.contains("gpt-4o") {
        return (0.0025, 0.010);
    }
    if m.contains("gpt-4-turbo") {
        return (0.010, 0.030);
    }
    if m.contains("gpt-4") {
        return (0.030, 0.060);
    }
    if m.contains("gpt-3.5") || m.contains("gpt-35") {
        return (0.0005, 0.0015);
    }
    if m.contains("o1-mini") {
        return (0.003, 0.012);
    }
    if m.contains("o1") {
        return (0.015, 0.060);
    }
    if m.contains("o3-mini") {
        return (0.0011, 0.0044);
    }

    // ── Google ───────────────────────────────────────────────────────────────
    if m.contains("gemini-2.0-flash") {
        return (0.000075, 0.0003);
    }
    if m.contains("gemini-1.5-flash") {
        return (0.000075, 0.0003);
    }
    if m.contains("gemini-1.5-pro") {
        return (0.00125, 0.005);
    }

    (0.0, 0.0)
}

/// Compute the USD cost for a single LLM response.
pub fn cost_for_response(response: &LlmResponse) -> f64 {
    let (input_per_k, output_per_k) = price_for_model(&response.model);
    let usage = match &response.usage {
        Some(u) => u,
        None => return 0.0,
    };
    (usage.prompt_tokens as f64 / 1_000.0) * input_per_k
        + (usage.completion_tokens as f64 / 1_000.0) * output_per_k
}

/// Accumulated token usage and cost across multiple LLM responses in a session.
#[derive(Debug, Clone, Default)]
pub struct SessionCostSummary {
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_cost_usd: f64,
    pub response_count: u32,
}

impl SessionCostSummary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a response to the running total.
    pub fn add(&mut self, response: &LlmResponse) {
        if let Some(usage) = &response.usage {
            self.total_prompt_tokens += usage.prompt_tokens as u64;
            self.total_completion_tokens += usage.completion_tokens as u64;
        }
        self.total_cost_usd += cost_for_response(response);
        self.response_count += 1;
    }

    /// Total tokens (prompt + completion) consumed so far.
    pub fn total_tokens(&self) -> u64 {
        self.total_prompt_tokens + self.total_completion_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wesichain_core::{LlmResponse, TokenUsage};

    fn sonnet_response(prompt: u32, completion: u32) -> LlmResponse {
        LlmResponse {
            model: "claude-3-5-sonnet-20241022".to_string(),
            content: String::new(),
            tool_calls: vec![],
            usage: Some(TokenUsage { prompt_tokens: prompt, completion_tokens: completion, total_tokens: prompt + completion }),
        }
    }

    #[test]
    fn cost_computation_is_correct() {
        // 1 000 input @ $0.003/1k + 500 output @ $0.015/1k = 0.003 + 0.0075 = 0.0105
        let r = sonnet_response(1_000, 500);
        let cost = cost_for_response(&r);
        assert!((cost - 0.0105).abs() < 1e-9, "cost = {cost}");
    }

    #[test]
    fn summary_accumulates() {
        let mut summary = SessionCostSummary::new();
        summary.add(&sonnet_response(1_000, 500));
        summary.add(&sonnet_response(1_000, 500));
        assert_eq!(summary.response_count, 2);
        assert_eq!(summary.total_prompt_tokens, 2_000);
        assert!((summary.total_cost_usd - 0.021).abs() < 1e-9);
    }

    #[test]
    fn unknown_model_zero_cost() {
        let r = LlmResponse {
            model: "my-local-model".to_string(),
            content: String::new(),
            tool_calls: vec![],
            usage: Some(TokenUsage { prompt_tokens: 1_000, completion_tokens: 500, total_tokens: 1_500 }),
        };
        assert_eq!(cost_for_response(&r), 0.0);
    }
}
