use crate::action::{ActionAgent, AgentStep};
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use wesichain_core::{LlmRequest, Message, Role, Runnable, Tool, ToolCall, WesichainError};

/// Legacy agent executor with known issues (hardcoded tool IDs, no ReAct loop).
///
/// Use `ReActAgentNode` from `wesichain-graph` for production use.
#[deprecated(
    since = "0.3.0",
    note = "Use ReActAgentNode from wesichain-graph for proper ReAct Thought/Action/Observation loop. Will be removed in v0.4.0."
)]
pub struct AgentExecutor<A> {
    agent: A,
    tools: HashMap<String, Box<dyn Tool>>,
    max_iterations: Option<usize>,
}

#[allow(deprecated)]
impl<A> AgentExecutor<A>
where
    A: ActionAgent,
{
    pub fn new(agent: A, tools: Vec<Box<dyn Tool>>) -> Self {
        let tools_map = tools
            .into_iter()
            .map(|t| (t.name().to_string(), t))
            .collect();
        Self {
            agent,
            tools: tools_map,
            max_iterations: Some(15),
        }
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = Some(max);
        self
    }
}

#[allow(deprecated)]
#[async_trait]
impl<A> Runnable<LlmRequest, String> for AgentExecutor<A>
where
    A: ActionAgent + Send + Sync,
{
    async fn invoke(&self, input: LlmRequest) -> Result<String, WesichainError> {
        let mut current_input = input;
        let mut iterations = 0;

        loop {
            if let Some(max) = self.max_iterations {
                if iterations >= max {
                    return Ok("Agent stopped due to iteration limit".to_string());
                }
            }
            iterations += 1;

            // 1. Invoke Agent
            // Note: Runnable<LlmRequest, AgentStep> is needed for A.
            // However, ActionAgent is Runnable<LlmResponse, AgentStep>.
            // We need to bridge LlmRequest -> LlmResponse via LLM inside the Agent?
            // Or typically Agent = Chain(Prompt | LLM | Parser).
            // So Agent input is whatever the Chain expects.
            // For now, let's assume ActionAgent handles LlmRequest?
            // Wait, definition in action.rs was Runnable<LlmResponse, AgentStep>.
            // That implies Agent is JUST the parser? No, LangChain Agent is the whole chain.
            // Let's adjust ActionAgent definition or AgentExecutor usage.
            // LangChain: Agent is Runnable<Input, AgentAction/Finish>.

            // For Wesichain, let's assume `agent` is a Runnable that takes `LlmRequest` (augmented with scratchpad)
            // and returns `AgentStep`.
            // But strict typing in Rust is tricky.
            // Let's define `AgentRunnable` trait: Runnable<LlmRequest, AgentStep>.
            // But our `ActionAgent` trait in `action.rs` was defined as `Runnable<LlmResponse, AgentStep>`.
            // Let's fix `action.rs` definition first or wrap it.

            // Actually, the `Agent` usually includes the LLM.
            // So `agent.invoke(custom_input) -> AgentStep`.
            // Here `custom_input` needs to include intermediate steps.
            // For simplicity, we'll use `LlmRequest` as the carrier of state (messages).

            // We need to call the agent. BUT compile-time check for traits is strict.
            // Ideally `self.agent` implements `Runnable<LlmRequest, AgentStep>`.
            // Let's assume we fix `ActionAgent` to be that.

            // For now, let's proceed assuming we will fix traits.
            let step = self.agent.invoke(current_input.clone()).await?;

            match step {
                AgentStep::Finish(finish) => {
                    return Ok(finish.return_values.to_string());
                }
                AgentStep::Action(action) => {
                    // 2. Execute Tool
                    let tool_name = action.tool.clone();
                    let output = if let Some(tool) = self.tools.get(&tool_name) {
                        tool.invoke(action.tool_input.clone())
                            .await
                            .unwrap_or_else(|e| Value::String(e.to_string()))
                    } else {
                        Value::String(format!("Tool {} not found", tool_name))
                    };

                    // 3. Update History (Scratchpad)
                    // We append AI message (Tool Call) and Tool Message (Result) to `current_input.messages`.
                    // This assumes `current_input` messages are mutable history.

                    // We need a way to represent the Tool Invocation in LlmRequest messages.
                    // LlmRequest messages are standard Human/AI/System/Tool.

                    // Add ToolCall (AI message)
                    let call_id = uuid::Uuid::new_v4().to_string();
                    current_input.messages.push(Message {
                        role: Role::Assistant,
                        content: "".to_string(),
                        tool_calls: vec![ToolCall {
                            id: call_id.clone(),
                            name: tool_name,
                            args: action.tool_input,
                        }],
                        tool_call_id: None,
                    });

                    // Add ToolOutput
                    current_input.messages.push(Message {
                        role: Role::Tool,
                        content: output.to_string(),
                        tool_calls: vec![],
                        tool_call_id: Some(call_id),
                    });
                }
            }
        }
    }

    // Stream not implemented for now
    fn stream(
        &self,
        _input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}
