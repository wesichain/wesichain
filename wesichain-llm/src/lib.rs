mod ollama;
#[cfg(feature = "openai")]
mod openai;

pub use ollama::{ollama_stream_events, OllamaClient};
pub use wesichain_core::{LlmRequest, LlmResponse, Message, Role, ToolCall, ToolSpec};

#[cfg(feature = "openai")]
pub use openai::OpenAiClient;

use wesichain_core::Runnable;

pub trait Llm: Runnable<LlmRequest, LlmResponse> {}

impl<T> Llm for T where T: Runnable<LlmRequest, LlmResponse> {}
