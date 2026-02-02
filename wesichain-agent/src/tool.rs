use std::collections::HashMap;

use wesichain_core::{Value, WesichainError};
use wesichain_llm::ToolSpec;

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    async fn call(&self, input: Value) -> Result<Value, WesichainError>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub async fn call(&self, name: &str, input: Value) -> Result<Value, WesichainError> {
        let tool = self.tools.get(name).ok_or_else(|| WesichainError::ToolCallFailed {
            tool_name: name.to_string(),
            reason: "not found".to_string(),
        })?;
        tool.call(input).await
    }

    pub fn to_specs(&self) -> Vec<ToolSpec> {
        self.tools
            .values()
            .map(|tool| ToolSpec {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.schema(),
            })
            .collect()
    }
}
