use std::sync::Arc;

use futures::stream::{self, BoxStream, StreamExt};
use wesichain_agent::Tool;
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_llm::{Message, Role, ToolCall};

use crate::{GraphState, StateSchema, StateUpdate};

pub trait HasToolCalls {
    fn tool_calls(&self) -> &Vec<ToolCall>;
    fn push_tool_result(&mut self, message: Message);
}

pub struct ToolNode {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolNode {
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        Self { tools }
    }

    pub async fn invoke<S>(&self, input: GraphState<S>) -> Result<StateUpdate<S>, WesichainError>
    where
        S: StateSchema + HasToolCalls,
    {
        <Self as Runnable<GraphState<S>, StateUpdate<S>>>::invoke(self, input).await
    }
}

#[async_trait::async_trait]
impl<S> Runnable<GraphState<S>, StateUpdate<S>> for ToolNode
where
    S: StateSchema + HasToolCalls,
{
    async fn invoke(&self, input: GraphState<S>) -> Result<StateUpdate<S>, WesichainError> {
        let mut next = input.data.clone();
        for call in input.data.tool_calls() {
            let tool = self
                .tools
                .iter()
                .find(|tool| tool.name() == call.name)
                .ok_or_else(|| WesichainError::ToolCallFailed {
                    tool_name: call.name.clone(),
                    reason: "not found".to_string(),
                })?;
            let output = tool
                .invoke(call.args.clone())
                .await
                .map_err(|err| WesichainError::ToolCallFailed {
                    tool_name: call.name.clone(),
                    reason: err.to_string(),
                })?;
            next.push_tool_result(Message {
                role: Role::Tool,
                content: output.to_string(),
                tool_call_id: Some(call.id.clone()),
                tool_calls: Vec::new(),
            });
        }
        Ok(StateUpdate::new(next))
    }

    fn stream(&self, _input: GraphState<S>) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::once(
            async move { Err(WesichainError::Custom("stream not implemented".to_string())) },
        )
        .boxed()
    }
}
