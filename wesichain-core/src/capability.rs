//! Model capability registry.
//!
//! Provides [`ModelCapabilities`] and [`for_model`] so callers can query what
//! a given model supports (tool use, vision, extended thinking, streaming, …)
//! without hard-coding checks throughout the codebase.
//!
//! # Example
//! ```
//! use wesichain_core::capability::for_model;
//!
//! let caps = for_model("claude-opus-4-6");
//! assert!(caps.tools);
//! assert!(caps.vision);
//! ```

/// Capability flags for a specific LLM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCapabilities {
    /// Model name (as passed to the API).
    pub model: String,
    /// Supports parallel tool / function calling.
    pub tools: bool,
    /// Accepts image inputs (vision).
    pub vision: bool,
    /// Supports extended thinking / reasoning traces.
    pub thinking: bool,
    /// Supports streaming responses.
    pub streaming: bool,
    /// Maximum context window in tokens (input + output combined).
    pub context_window: u32,
    /// Default maximum output tokens (may be raised up to `context_window`).
    pub max_output_tokens: u32,
    /// Provider name, e.g. `"anthropic"`, `"openai"`.
    pub provider: &'static str,
}

impl ModelCapabilities {
    fn anthropic(
        model: &str,
        tools: bool,
        vision: bool,
        thinking: bool,
        context_window: u32,
        max_output_tokens: u32,
    ) -> Self {
        Self {
            model: model.to_string(),
            tools,
            vision,
            thinking,
            streaming: true,
            context_window,
            max_output_tokens,
            provider: "anthropic",
        }
    }

    fn openai(
        model: &str,
        tools: bool,
        vision: bool,
        thinking: bool,
        context_window: u32,
        max_output_tokens: u32,
    ) -> Self {
        Self {
            model: model.to_string(),
            tools,
            vision,
            thinking,
            streaming: true,
            context_window,
            max_output_tokens,
            provider: "openai",
        }
    }
}

/// Look up capabilities for a known model identifier.
///
/// Returns a best-effort [`ModelCapabilities`] for the given model string.
/// For unknown models a safe default (tools=false, vision=false, thinking=false)
/// is returned so callers can degrade gracefully rather than panic.
pub fn for_model(model: &str) -> ModelCapabilities {
    // Normalise: strip version suffixes like "-20241022" or "@latest".
    let normalised = model.to_lowercase();
    let norm = normalised.as_str();

    // ── Anthropic ────────────────────────────────────────────────────────────

    // Claude 4 / claude-opus-4 family
    if norm.contains("claude-opus-4") {
        return ModelCapabilities::anthropic(model, true, true, false, 200_000, 32_000);
    }
    if norm.contains("claude-sonnet-4") {
        return ModelCapabilities::anthropic(model, true, true, false, 200_000, 64_000);
    }

    // Claude 3.7 Sonnet — extended thinking support
    if norm.contains("claude-3-7-sonnet") || norm.contains("claude-3.7-sonnet") {
        return ModelCapabilities::anthropic(model, true, true, true, 200_000, 64_000);
    }

    // Claude 3.5 family
    if norm.contains("claude-3-5-sonnet") || norm.contains("claude-3.5-sonnet") {
        return ModelCapabilities::anthropic(model, true, true, false, 200_000, 8_192);
    }
    if norm.contains("claude-3-5-haiku") || norm.contains("claude-3.5-haiku") {
        return ModelCapabilities::anthropic(model, true, true, false, 200_000, 8_192);
    }

    // Claude 3 family
    if norm.contains("claude-3-opus") {
        return ModelCapabilities::anthropic(model, true, true, false, 200_000, 4_096);
    }
    if norm.contains("claude-3-sonnet") {
        return ModelCapabilities::anthropic(model, true, true, false, 200_000, 4_096);
    }
    if norm.contains("claude-3-haiku") {
        return ModelCapabilities::anthropic(model, true, true, false, 200_000, 4_096);
    }

    // Claude 2 family (no vision)
    if norm.contains("claude-2") || norm.contains("claude-instant") {
        return ModelCapabilities::anthropic(model, false, false, false, 100_000, 4_096);
    }

    // ── OpenAI ───────────────────────────────────────────────────────────────

    if norm.contains("gpt-4o") {
        return ModelCapabilities::openai(model, true, true, false, 128_000, 16_384);
    }
    if norm.contains("gpt-4-turbo") || norm.contains("gpt-4t") {
        return ModelCapabilities::openai(model, true, true, false, 128_000, 4_096);
    }
    if norm.contains("gpt-4") {
        return ModelCapabilities::openai(model, true, false, false, 8_192, 4_096);
    }
    if norm.contains("gpt-3.5") || norm.contains("gpt-35") {
        return ModelCapabilities::openai(model, true, false, false, 16_384, 4_096);
    }
    if norm.contains("o1") || norm.contains("o3") {
        return ModelCapabilities::openai(model, true, true, true, 200_000, 100_000);
    }

    // ── Google ───────────────────────────────────────────────────────────────

    if norm.contains("gemini-2") {
        return ModelCapabilities {
            model: model.to_string(),
            tools: true,
            vision: true,
            thinking: norm.contains("flash-thinking"),
            streaming: true,
            context_window: 1_000_000,
            max_output_tokens: 8_192,
            provider: "google",
        };
    }
    if norm.contains("gemini-1.5") {
        return ModelCapabilities {
            model: model.to_string(),
            tools: true,
            vision: true,
            thinking: false,
            streaming: true,
            context_window: 1_000_000,
            max_output_tokens: 8_192,
            provider: "google",
        };
    }

    // ── Unknown fallback ─────────────────────────────────────────────────────

    ModelCapabilities {
        model: model.to_string(),
        tools: false,
        vision: false,
        thinking: false,
        streaming: false,
        context_window: 4_096,
        max_output_tokens: 4_096,
        provider: "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_37_sonnet_supports_thinking() {
        let caps = for_model("claude-3-7-sonnet-20250219");
        assert!(caps.thinking);
        assert!(caps.tools);
        assert!(caps.vision);
        assert_eq!(caps.provider, "anthropic");
    }

    #[test]
    fn claude_35_sonnet_no_thinking() {
        let caps = for_model("claude-3-5-sonnet-20241022");
        assert!(!caps.thinking);
        assert!(caps.tools);
        assert!(caps.vision);
    }

    #[test]
    fn gpt4o_caps() {
        let caps = for_model("gpt-4o");
        assert!(caps.tools);
        assert!(caps.vision);
        assert!(!caps.thinking);
        assert_eq!(caps.provider, "openai");
    }

    #[test]
    fn unknown_model_safe_defaults() {
        let caps = for_model("my-custom-model");
        assert!(!caps.tools);
        assert!(!caps.thinking);
        assert_eq!(caps.provider, "unknown");
    }

    #[test]
    fn gemini_2_flash_thinking() {
        let caps = for_model("gemini-2-flash-thinking-exp");
        assert!(caps.thinking);
        assert_eq!(caps.provider, "google");
    }
}
