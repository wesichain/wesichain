mod ollama;
mod types;

pub use ollama::{ollama_stream_events, OllamaClient};
pub use types::{LlmRequest, LlmResponse, Message, Role, ToolCall, ToolSpec};

use wesichain_core::Runnable;

pub trait Llm: Runnable<LlmRequest, LlmResponse> {}

impl<T> Llm for T where T: Runnable<LlmRequest, LlmResponse> {}
