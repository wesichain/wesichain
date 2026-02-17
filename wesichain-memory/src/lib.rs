use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use wesichain_core::WesichainError;

pub mod buffer;
mod buffer_tests;
pub mod summary;
mod summary_tests;
pub mod window;
mod window_tests;

#[async_trait]
pub trait Memory: Send + Sync {
    /// Return key-value pairs of memory variables (e.g., chat history)
    async fn load_memory_variables(
        &self,
        thread_id: &str,
    ) -> Result<HashMap<String, Value>, WesichainError>;

    /// Save context from this interaction to memory
    async fn save_context(
        &self,
        thread_id: &str,
        inputs: &HashMap<String, Value>,
        outputs: &HashMap<String, Value>,
    ) -> Result<(), WesichainError>;

    /// Clear memory for the given thread
    async fn clear(&self, thread_id: &str) -> Result<(), WesichainError>;
}
