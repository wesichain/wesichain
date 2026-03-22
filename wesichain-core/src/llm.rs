use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{TokenUsage, Value, WesichainError};
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
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { url: String, detail: Option<String> },
    ImageData { data: String, media_type: String },
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl Default for MessageContent {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

impl std::fmt::Display for MessageContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_text_lossy())
    }
}

impl MessageContent {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(s) => Some(s.as_str()),
            MessageContent::Parts(_) => None,
        }
    }

    pub fn to_text_lossy(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Parts(parts) => parts
                .iter()
                .filter_map(|p| {
                    if let ContentPart::Text { text } = p {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(""),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            MessageContent::Text(s) => s.is_empty(),
            MessageContent::Parts(parts) => parts.is_empty(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

impl Message {
    pub fn user(content: impl Into<MessageContent>) -> Self {
        Self { role: Role::User, content: content.into(), tool_call_id: None, tool_calls: vec![] }
    }

    pub fn system(content: impl Into<MessageContent>) -> Self {
        Self { role: Role::System, content: content.into(), tool_call_id: None, tool_calls: vec![] }
    }

    pub fn assistant(content: impl Into<MessageContent>) -> Self {
        Self { role: Role::Assistant, content: content.into(), tool_call_id: None, tool_calls: vec![] }
    }

    pub fn with_image_url(mut self, url: impl Into<String>, detail: Option<String>) -> Self {
        let parts = match self.content {
            MessageContent::Text(t) if !t.is_empty() => vec![
                ContentPart::Text { text: t },
                ContentPart::ImageUrl { url: url.into(), detail },
            ],
            MessageContent::Text(_) => vec![ContentPart::ImageUrl { url: url.into(), detail }],
            MessageContent::Parts(mut parts) => {
                parts.push(ContentPart::ImageUrl { url: url.into(), detail });
                parts
            }
        };
        self.content = MessageContent::Parts(parts);
        self
    }

    pub fn with_image_data(mut self, data: impl Into<String>, media_type: impl Into<String>) -> Self {
        let parts = match self.content {
            MessageContent::Text(t) if !t.is_empty() => vec![
                ContentPart::Text { text: t },
                ContentPart::ImageData { data: data.into(), media_type: media_type.into() },
            ],
            MessageContent::Text(_) => vec![ContentPart::ImageData { data: data.into(), media_type: media_type.into() }],
            MessageContent::Parts(mut parts) => {
                parts.push(ContentPart::ImageData { data: data.into(), media_type: media_type.into() });
                parts
            }
        };
        self.content = MessageContent::Parts(parts);
        self
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop_sequences: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct LlmResponse {
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    /// Model identifier returned by the provider (e.g. `"claude-3-5-sonnet-20241022"`).
    /// Empty string when the provider did not return a model name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
}

#[async_trait]
pub trait ToolCallingLlm: crate::Runnable<LlmRequest, LlmResponse> + Send + Sync + 'static {}

impl crate::Bindable for LlmRequest {
    fn bind(&mut self, args: Value) -> Result<(), WesichainError> {
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

        let tool_spec = ToolSpec {
            name: "output_formatter".to_string(),
            description: "Output the result in this format".to_string(),
            parameters: as_value,
        };

        let bound = self.bind(serde_json::json!({
            "tools": [tool_spec]
        }));

        bound.then(StructuredOutputParser::<T>::new())
    }
}

impl<L> ToolCallingLlmExt for L where L: ToolCallingLlm {}
