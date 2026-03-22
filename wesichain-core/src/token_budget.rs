//! Token budget cap for LLM context windows.
//!
//! [`TokenBudget`] trims the oldest non-system messages from a [`Vec<Message>`]
//! to keep the estimated token count within a configured limit, preventing
//! context-overflow errors from the provider.
//!
//! Token counts are estimated at ~4 characters per token (standard heuristic).
//! For exact counts, providers should return `usage` in `LlmResponse` instead.
//!
//! # Example
//! ```
//! use wesichain_core::{Message, token_budget::TokenBudget};
//!
//! let budget = TokenBudget::new(8_192);
//! let mut messages = vec![
//!     Message::system("You are helpful."),
//!     Message::user("Question 1"),
//!     Message::user("Question 2"),
//! ];
//! budget.apply(&mut messages);
//! // messages may be trimmed to fit within 8 192 tokens
//! ```

use crate::{Message, Role};

const CHARS_PER_TOKEN: usize = 4;

/// A per-session token budget that trims oldest messages to stay within a limit.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// Maximum number of tokens allowed across all messages.
    pub max_tokens: usize,
}

impl TokenBudget {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens }
    }

    /// Estimate the token count for a single message.
    fn estimate_tokens(msg: &Message) -> usize {
        let chars = msg.content.to_text_lossy().len();
        // Add a small per-message overhead (role + formatting).
        4 + chars / CHARS_PER_TOKEN
    }

    /// Return the estimated total token count for a slice of messages.
    pub fn estimate_total(messages: &[Message]) -> usize {
        messages.iter().map(Self::estimate_tokens).sum()
    }

    /// Trim `messages` in-place by removing the oldest non-system messages
    /// until the estimated token count fits within `self.max_tokens`.
    ///
    /// System messages are never removed.  If even the system messages alone
    /// exceed the budget the function returns without trimming (avoids leaving
    /// an empty or incoherent context).
    pub fn apply(&self, messages: &mut Vec<Message>) {
        while Self::estimate_total(messages) > self.max_tokens {
            // Find the first non-system message index.
            let first_non_system = messages
                .iter()
                .position(|m| !matches!(m.role, Role::System));

            match first_non_system {
                Some(idx) => {
                    messages.remove(idx);
                }
                None => {
                    // Only system messages remain — stop to avoid a vacuous context.
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Message;

    #[test]
    fn trims_oldest_messages_to_fit() {
        let budget = TokenBudget::new(50);

        // Each message is short enough individually, but together they exceed 50 tokens.
        let long = "x".repeat(200); // ~50 tokens each
        let mut messages = vec![
            Message::system("sys"),
            Message::user(long.clone()),
            Message::user(long.clone()),
            Message::user("short"),
        ];

        budget.apply(&mut messages);

        // System message must be preserved.
        assert!(messages.iter().any(|m| matches!(m.role, Role::System)));
        // Total must now be within budget.
        assert!(TokenBudget::estimate_total(&messages) <= 50);
    }

    #[test]
    fn does_not_trim_system_messages() {
        let budget = TokenBudget::new(1); // impossibly small
        let mut messages = vec![Message::system("system prompt")];
        budget.apply(&mut messages);
        // Should not remove system-only messages even if over budget.
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn noop_when_within_budget() {
        let budget = TokenBudget::new(10_000);
        let mut messages = vec![
            Message::system("sys"),
            Message::user("hello"),
        ];
        let original_len = messages.len();
        budget.apply(&mut messages);
        assert_eq!(messages.len(), original_len);
    }
}
