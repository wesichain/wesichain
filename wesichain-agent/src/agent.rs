use futures::stream::{self, BoxStream, StreamExt};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_llm::{LlmRequest, LlmResponse, Message, Role};

use crate::ToolRegistry;

pub struct ToolCallingAgent<L> {
    llm: L,
    tools: ToolRegistry,
    model: String,
    max_steps: usize,
}

impl<L> ToolCallingAgent<L> {
    pub fn new(llm: L, tools: ToolRegistry, model: String) -> Self {
        Self {
            llm,
            tools,
            model,
            max_steps: 5,
        }
    }

    pub fn max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }
}

#[async_trait::async_trait]
impl<L> Runnable<String, String> for ToolCallingAgent<L>
where
    L: Runnable<LlmRequest, LlmResponse> + Send + Sync,
{
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        let mut messages = vec![Message {
            role: Role::User,
            content: input,
            tool_call_id: None,
        }];

        for _ in 0..self.max_steps {
            let tool_specs = self.tools.to_specs();
            let response = self
                .llm
                .invoke(LlmRequest {
                    model: self.model.clone(),
                    messages: messages.clone(),
                    tools: tool_specs,
                })
                .await?;
            if response.tool_calls.is_empty() {
                return Ok(response.content);
            }

            messages.push(Message {
                role: Role::Assistant,
                content: response.content,
                tool_call_id: None,
            });

            for call in response.tool_calls {
                let result = self.tools.call(&call.name, call.args).await?;
                messages.push(Message {
                    role: Role::Tool,
                    content: result.to_string(),
                    tool_call_id: Some(call.id.clone()),
                });
            }
        }

        Err(WesichainError::Custom(format!(
            "max steps exceeded: {}",
            self.max_steps
        )))
    }

    fn stream(&self, _input: String) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::once(
            async move { Err(WesichainError::Custom("stream not implemented".to_string())) },
        )
        .boxed()
    }
}
