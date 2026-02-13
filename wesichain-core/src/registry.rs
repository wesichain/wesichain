use crate::tool::Tool;
use crate::{Runnable, WesichainError};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

type ToolFactory = Box<dyn Fn(Value) -> Result<Arc<dyn Tool>, WesichainError> + Send + Sync>;
type LlmFactory = Box<
    dyn Fn(
            HashMap<String, Value>,
        )
            -> Result<Arc<dyn Runnable<crate::LlmRequest, crate::LlmResponse>>, WesichainError>
        + Send
        + Sync,
>;
type PromptFactory = Box<
    dyn Fn(String, Vec<String>) -> Result<Arc<dyn Runnable<Value, Value>>, WesichainError>
        + Send
        + Sync,
>;

#[derive(Default)]
pub struct RunnableRegistry {
    tool_factories: HashMap<String, ToolFactory>,
    llm_factories: HashMap<String, LlmFactory>,
    prompt_factories: HashMap<String, PromptFactory>,
}

impl RunnableRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_tool<F>(&mut self, name: &str, factory: F)
    where
        F: Fn(Value) -> Result<Arc<dyn Tool>, WesichainError> + Send + Sync + 'static,
    {
        self.tool_factories
            .insert(name.to_string(), Box::new(factory));
    }

    pub fn register_llm<F>(&mut self, name: &str, factory: F)
    where
        F: Fn(
                HashMap<String, Value>,
            )
                -> Result<Arc<dyn Runnable<crate::LlmRequest, crate::LlmResponse>>, WesichainError>
            + Send
            + Sync
            + 'static,
    {
        self.llm_factories
            .insert(name.to_string(), Box::new(factory));
    }

    pub fn register_prompt<F>(&mut self, name: &str, factory: F)
    where
        F: Fn(String, Vec<String>) -> Result<Arc<dyn Runnable<Value, Value>>, WesichainError>
            + Send
            + Sync
            + 'static,
    {
        self.prompt_factories
            .insert(name.to_string(), Box::new(factory));
    }

    pub fn lookup_tool(&self, name: &str, config: Value) -> Result<Arc<dyn Tool>, WesichainError> {
        if let Some(factory) = self.tool_factories.get(name) {
            factory(config)
        } else {
            Err(WesichainError::Custom(format!(
                "Tool '{}' not found in registry",
                name
            )))
        }
    }

    pub fn lookup_prompt(
        &self,
        template: &str,
        input_variables: Vec<String>,
    ) -> Result<Arc<dyn Runnable<Value, Value>>, WesichainError> {
        if let Some(factory) = self.prompt_factories.get("default") {
            factory(template.to_string(), input_variables)
        } else {
            Err(WesichainError::Custom(
                "No default prompt factory registered".to_string(),
            ))
        }
    }

    pub fn lookup_llm(
        &self,
        name: &str,
        config: HashMap<String, Value>,
    ) -> Result<Arc<dyn Runnable<crate::LlmRequest, crate::LlmResponse>>, WesichainError> {
        if let Some(factory) = self.llm_factories.get(name) {
            factory(config)
        } else {
            Err(WesichainError::Custom(format!(
                "LLM '{}' not found in registry",
                name
            )))
        }
    }
}
