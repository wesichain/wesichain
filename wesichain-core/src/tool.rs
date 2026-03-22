use crate::Value;
use thiserror::Error;

pub use tokio_util::sync::CancellationToken;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    async fn invoke(&self, args: Value) -> Result<Value, ToolError>;
}

#[derive(Clone, Debug)]
pub struct ToolContext {
    pub correlation_id: String,
    pub step_id: u32,
    pub cancellation: CancellationToken,
    /// Optional channel for streaming tool output to the agent event loop in real time.
    ///
    /// Tools that produce incremental output (e.g. `BashExecTool`) should send
    /// `StreamEvent::ContentChunk` items here so the host can display progress
    /// without waiting for the full result.
    ///
    /// Set to `None` if no streaming consumer is attached.
    pub stream_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::StreamEvent>>,
}

impl ToolContext {
    /// Convenience constructor that leaves `stream_tx` disconnected.
    pub fn new(
        correlation_id: impl Into<String>,
        step_id: u32,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            correlation_id: correlation_id.into(),
            step_id,
            cancellation,
            stream_tx: None,
        }
    }
}

#[async_trait::async_trait]
pub trait TypedTool: Send + Sync {
    type Args: serde::de::DeserializeOwned + schemars::JsonSchema + Send;
    type Output: serde::Serialize + schemars::JsonSchema + Send;
    const NAME: &'static str;
    async fn run(&self, args: Self::Args, ctx: ToolContext) -> Result<Self::Output, ToolError>;
}
