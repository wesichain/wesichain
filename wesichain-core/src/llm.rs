use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{Value, WesichainError};
use async_trait::async_trait;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LlmResponse {
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

#[async_trait]
pub trait ToolCallingLlm: crate::Runnable<LlmRequest, LlmResponse> + Send + Sync + 'static {}

impl crate::Bindable for LlmRequest {
    fn bind(&mut self, args: Value) -> Result<(), WesichainError> {
        // We only support binding "tools" for now, which is a list of tool specs
        if let Some(obj) = args.as_object() {
            if let Some(tools_val) = obj.get("tools") {
                let tools: Vec<ToolSpec> =
                    serde_json::from_value(tools_val.clone()).map_err(WesichainError::Serde)?;
                self.tools.extend(tools);
            }
        }
        Ok(())
    }
}

pub trait ToolCallingLlmExt: ToolCallingLlm {
    fn with_structured_output<T>(self) -> impl crate::Runnable<LlmRequest, T>
    where
        T: schemars::JsonSchema + DeserializeOwned + Serialize + Send + Sync + 'static,
        Self: Sized,
    {
        use crate::{RunnableExt, StructuredOutputParser};

        let schema = schemars::schema_for!(T);
        let as_value = serde_json::to_value(schema).unwrap_or(Value::Null);

        // Wrap in a tool spec
        let tool_spec = ToolSpec {
            name: "output_formatter".to_string(), // Fixed name for now, or derive from T type name?
            description: "Output the result in this format".to_string(),
            parameters: as_value,
        };

        // Bind the tool to the LLM
        let bound = self.bind(serde_json::json!({
            "tools": [tool_spec]
        }));

        // Chain with parser
        bound.then(StructuredOutputParser::<T>::new())
    }
}

impl<L> ToolCallingLlmExt for L where L: ToolCallingLlm {}
