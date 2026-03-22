use std::sync::Arc;

use futures::stream::{self, BoxStream, StreamExt};
use tokio::task::JoinSet;
use wesichain_core::Tool;
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_llm::{Message, Role, ToolCall};

use crate::{GraphState, StateSchema, StateUpdate};

/// Trait for states that contain pending tool calls.
///
/// Implement this on your state to use [`ToolNode`] for generic tool execution.
/// For ReAct-style agents using scratchpad-based state, use
/// [`ReActToolNode`](crate::react_subgraph::ReActToolNode) with `ScratchpadState` instead.
pub trait HasToolCalls {
    fn tool_calls(&self) -> &Vec<ToolCall>;
    fn push_tool_result(&mut self, message: Message);
}

/// Generic tool execution node for graph-based workflows.
///
/// `ToolNode` executes all pending tool calls from state via the [`HasToolCalls`] trait.
/// This is the general-purpose tool executor suitable for any workflow.
///
/// For ReAct-style agents, prefer [`ReActToolNode`](crate::react_subgraph::ReActToolNode)
/// which integrates with the scratchpad pattern (`ScratchpadState`).
pub struct ToolNode {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolNode {
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        Self { tools }
    }

    pub async fn invoke<S>(&self, input: GraphState<S>) -> Result<StateUpdate<S>, WesichainError>
    where
        S: StateSchema<Update = S> + HasToolCalls,
    {
        <Self as Runnable<GraphState<S>, StateUpdate<S>>>::invoke(self, input).await
    }
}

#[async_trait::async_trait]
impl<S> Runnable<GraphState<S>, StateUpdate<S>> for ToolNode
where
    S: StateSchema<Update = S> + HasToolCalls,
{
    async fn invoke(&self, input: GraphState<S>) -> Result<StateUpdate<S>, WesichainError> {
        let calls: Vec<ToolCall> = input.data.tool_calls().clone();

        // Dispatch all tool calls concurrently, preserving original order.
        let mut join_set: JoinSet<(usize, String, Result<String, WesichainError>)> =
            JoinSet::new();

        for (index, call) in calls.iter().enumerate() {
            let tool = self
                .tools
                .iter()
                .find(|t| t.name() == call.name)
                .ok_or_else(|| WesichainError::ToolCallFailed {
                    tool_name: call.name.clone(),
                    reason: "not found".to_string(),
                })?;
            let tool = tool.clone();
            let args = call.args.clone();
            let call_id = call.id.clone();
            let tool_name = call.name.clone();
            join_set.spawn(async move {
                let result = tool.invoke(args).await.map(|v| v.to_string()).map_err(|e| {
                    WesichainError::ToolCallFailed {
                        tool_name,
                        reason: e.to_string(),
                    }
                });
                (index, call_id, result)
            });
        }

        // Collect results and sort by original index so message history is deterministic.
        let mut results: Vec<(usize, String, Result<String, WesichainError>)> =
            Vec::with_capacity(calls.len());
        while let Some(res) = join_set.join_next().await {
            results.push(res.map_err(|e| WesichainError::Custom(format!("task panicked: {e}")))?);
        }
        results.sort_by_key(|(idx, _, _)| *idx);

        let mut next = input.data.clone();
        for (_, call_id, output) in results {
            next.push_tool_result(Message {
                role: Role::Tool,
                content: output?.into(),
                tool_call_id: Some(call_id),
                tool_calls: Vec::new(),
            });
        }
        Ok(StateUpdate::new(next))
    }

    fn stream(&self, _input: GraphState<S>) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::empty().boxed()
    }
}
