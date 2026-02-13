use std::collections::HashMap;

use crate::PromptTemplate;
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use wesichain_core::{Message, Role, Runnable, StreamEvent, Value, WesichainError};

#[derive(Debug, Clone)]
pub enum MessagePromptTemplate {
    Human(PromptTemplate),
    AI(PromptTemplate),
    System(PromptTemplate),
    Placeholder { variable_name: String },
}

impl MessagePromptTemplate {
    pub fn human(template: &str) -> Self {
        Self::Human(PromptTemplate::new(template.to_string()))
    }

    pub fn ai(template: &str) -> Self {
        Self::AI(PromptTemplate::new(template.to_string()))
    }

    pub fn system(template: &str) -> Self {
        Self::System(PromptTemplate::new(template.to_string()))
    }

    pub fn placeholder(variable_name: &str) -> Self {
        Self::Placeholder {
            variable_name: variable_name.to_string(),
        }
    }

    pub fn format(&self, vars: &HashMap<String, Value>) -> Result<Vec<Message>, WesichainError> {
        match self {
            MessagePromptTemplate::Human(t) => Ok(vec![Message {
                role: Role::User,
                content: t.render(vars)?,
                tool_call_id: None,
                tool_calls: vec![],
            }]),
            MessagePromptTemplate::AI(t) => Ok(vec![Message {
                role: Role::Assistant,
                content: t.render(vars)?,
                tool_call_id: None,
                tool_calls: vec![],
            }]),
            MessagePromptTemplate::System(t) => Ok(vec![Message {
                role: Role::System,
                content: t.render(vars)?,
                tool_call_id: None,
                tool_calls: vec![],
            }]),
            MessagePromptTemplate::Placeholder { variable_name } => {
                if let Some(val) = vars.get(variable_name) {
                    // Expecting list of messages or single message
                    if let Ok(msgs) = serde_json::from_value::<Vec<Message>>(val.clone()) {
                        Ok(msgs)
                    } else if let Ok(msg) = serde_json::from_value::<Message>(val.clone()) {
                        Ok(vec![msg])
                    } else {
                        // Simplify: treat as string content for a user message if not message object?
                        // LangChain typically expects MessagesPlaceholder to fill with Messages.
                        // Only error if we can't parse as messages.
                        Ok(vec![])
                    }
                } else {
                    Ok(vec![])
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatPromptTemplate {
    messages: Vec<MessagePromptTemplate>,
}

impl ChatPromptTemplate {
    pub fn new(messages: Vec<MessagePromptTemplate>) -> Self {
        Self { messages }
    }

    pub fn format_messages(
        &self,
        vars: &HashMap<String, Value>,
    ) -> Result<Vec<Message>, WesichainError> {
        let mut result = Vec::new();
        for tmpl in &self.messages {
            result.extend(tmpl.format(vars)?);
        }
        Ok(result)
    }
}

#[async_trait]
impl Runnable<HashMap<String, Value>, Vec<Message>> for ChatPromptTemplate {
    async fn invoke(&self, input: HashMap<String, Value>) -> Result<Vec<Message>, WesichainError> {
        self.format_messages(&input)
    }

    fn stream(
        &self,
        input: HashMap<String, Value>,
    ) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::once(async move {
            let msgs = self.format_messages(&input)?;
            // What to stream? Maybe just the result wrapper?
            // Prompts usually don't stream, but they are Runnables.
            // We can emit a Metadata event or similar.
            Ok(StreamEvent::Metadata {
                key: "prompt".to_string(),
                value: serde_json::to_value(msgs).unwrap(),
            })
        })
        .boxed()
    }
}
