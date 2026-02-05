mod ollama;

// OpenAI-compatible client (always available)
pub mod openai_compatible;

// Provider-specific clients (feature-gated)
pub mod providers;

pub use ollama::{ollama_stream_events, OllamaClient};
pub use wesichain_core::{LlmRequest, LlmResponse, Message, Role, ToolCall, ToolSpec};

// Re-export generic client
pub use openai_compatible::{
    ChatCompletionRequest, OpenAiCompatibleBuilder, OpenAiCompatibleClient,
};

// Re-export provider clients
#[cfg(feature = "openai")]
pub use providers::openai::OpenAiClient;

#[cfg(feature = "deepseek")]
pub use providers::deepseek::DeepSeekClient;

use wesichain_core::Runnable;

pub trait Llm: Runnable<LlmRequest, LlmResponse> {}

impl<T> Llm for T where T: Runnable<LlmRequest, LlmResponse> {}
