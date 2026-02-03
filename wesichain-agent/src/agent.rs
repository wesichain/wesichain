use futures::stream::{self, BoxStream, StreamExt};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_llm::{LlmRequest, LlmResponse, Message, Role};

use crate::ToolRegistry;

#[deprecated(since = "0.1.0", note = "Use ReActAgentNode in wesichain-graph")]
pub struct ToolCallingAgent<L> {
    llm: L,
    tools: ToolRegistry,
    model: String,
    max_steps: usize,
}

#[allow(deprecated)]
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

#[allow(deprecated)]
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
            tool_calls: Vec::new(),
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
            let LlmResponse {
                content,
                tool_calls,
            } = response;
            if tool_calls.is_empty() {
                return Ok(content);
            }

            messages.push(Message {
                role: Role::Assistant,
                content,
                tool_call_id: None,
                tool_calls: tool_calls.clone(),
            });

            for call in tool_calls {
                let result = self
                    .tools
                    .call(&call.name, call.args)
                    .await
                    .map_err(|err| WesichainError::ToolCallFailed {
                        tool_name: call.name.clone(),
                        reason: err.to_string(),
                    })?;
                messages.push(Message {
                    role: Role::Tool,
                    content: result.to_string(),
                    tool_call_id: Some(call.id.clone()),
                    tool_calls: Vec::new(),
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
