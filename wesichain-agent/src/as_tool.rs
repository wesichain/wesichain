//! `AgentAsTool` — wrap any streaming agent so it can be called as a Tool.
//!
//! This lets one agent call another by name using the standard `Tool` dispatch
//! path, enabling agent-calls-agent patterns without a full supervisor graph.
//!
//! # Example
//! ```ignore
//! use wesichain_agent::as_tool::AgentAsTool;
//! use wesichain_agent::ToolSet;
//! use futures::stream;
//!
//! let research_agent = AgentAsTool::new(
//!     "researcher",
//!     "Search the web and return a summary",
//!     Arc::new(|task: String| {
//!         Box::pin(stream::once(async move {
//!             Ok(wesichain_core::StreamEvent::FinalAnswer(format!("Research: {task}")))
//!         })) as BoxStream<'static, _>
//!     }),
//! );
//!
//! let tools = ToolSet::new()
//!     .register_dynamic(research_agent)
//!     .build()?;
//! ```

use std::sync::Arc;

use futures::stream::BoxStream;
use serde_json::{json, Value};
use wesichain_core::{StreamEvent, ToolError, WesichainError};

/// A factory that creates a new stream for each invocation.
pub type AgentFactory = Arc<
    dyn Fn(String) -> BoxStream<'static, Result<StreamEvent, WesichainError>> + Send + Sync,
>;

/// Wraps a streaming agent as a [`wesichain_core::Tool`].
///
/// Since [`TypedTool`] requires a `const NAME`, dynamic names are handled by
/// implementing [`wesichain_core::Tool`] directly via [`DynamicTool`].
#[derive(Clone)]
pub struct AgentAsTool {
    pub name: String,
    pub description: String,
    factory: AgentFactory,
}

impl AgentAsTool {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        factory: AgentFactory,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            factory,
        }
    }

    /// Invoke the agent with a task string and collect the full output.
    pub async fn invoke_agent(&self, task: String) -> Result<String, WesichainError> {
        use futures::StreamExt;
        let mut stream = (self.factory)(task);
        let mut buf = String::new();
        while let Some(item) = stream.next().await {
            match item? {
                StreamEvent::ContentChunk(s) | StreamEvent::FinalAnswer(s) => buf.push_str(&s),
                _ => {}
            }
        }
        Ok(buf)
    }
}

/// Dynamic tool wrapping [`AgentAsTool`] — implements `wesichain_core::Tool`.
#[async_trait::async_trait]
impl wesichain_core::Tool for AgentAsTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The task or question to send to this agent"
                }
            },
            "required": ["task"]
        })
    }

    async fn invoke(&self, args: Value) -> Result<Value, ToolError> {
        let task = args
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("missing 'task' field".to_string()))?
            .to_string();

        let output = self.invoke_agent(task).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("agent '{name}' failed: {e}", name = self.name))
        })?;

        Ok(json!({ "output": output }))
    }
}
