#![allow(deprecated)]
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use wesichain_core::{
    HasFinalOutput, HasUserInput, LlmRequest, LlmResponse, Message, ReActStep, Role,
    ScratchpadState, Tool, ToolCall, ToolCallingLlm, ToolSpec, Value, WesichainError,
};
use wesichain_prompt::PromptTemplate;

use crate::error::GraphError;
use crate::graph::{GraphContext, GraphNode};
use crate::state::{GraphState, StateSchema, StateUpdate};

const DEFAULT_SYSTEM_PROMPT: &str = "You are a helpful assistant. Use tools when helpful. If a tool is used, wait for the tool result before answering.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolFailurePolicy {
    #[default]
    FailFast,
    AppendErrorAndContinue,
}

#[deprecated(
    since = "0.2.0",
    note = "Monolithic ReActAgentNode is deprecated. Use composable ReActGraphBuilder + AgentNode + ReActToolNode instead."
)]
pub struct ReActAgentNode {
    llm: Arc<dyn ToolCallingLlm>,
    tools: HashMap<String, Arc<dyn Tool>>,
    tool_specs: Vec<ToolSpec>,
    prompt: PromptTemplate,
    max_iterations: usize,
    tool_failure_policy: ToolFailurePolicy,
}

pub struct ReActAgentNodeBuilder {
    llm: Option<Arc<dyn ToolCallingLlm>>,
    tools: Vec<Arc<dyn Tool>>,
    prompt: PromptTemplate,
    max_iterations: usize,
    tool_failure_policy: ToolFailurePolicy,
}

impl ReActAgentNode {
    pub fn builder() -> ReActAgentNodeBuilder {
        ReActAgentNodeBuilder {
            llm: None,
            tools: Vec::new(),
            prompt: PromptTemplate::new(DEFAULT_SYSTEM_PROMPT.to_string()),
            max_iterations: 12,
            tool_failure_policy: ToolFailurePolicy::FailFast,
        }
    }

    fn build_messages<S>(&self, state: &S) -> Result<Vec<Message>, WesichainError>
    where
        S: ScratchpadState + HasUserInput,
    {
        let mut messages = Vec::new();
        let prompt = self.prompt.render(&HashMap::new())?;
        messages.push(Message {
            role: Role::System,
            content: prompt,
            tool_call_id: None,
            tool_calls: Vec::new(),
        });
        messages.push(Message {
            role: Role::User,
            content: state.user_input().to_string(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        });

        let mut pending_tool_calls: VecDeque<ToolCall> = VecDeque::new();
        let mut pending_thought: Option<String> = None;

        for step in state.scratchpad() {
            match step {
                ReActStep::Thought(text) => {
                    if let Some(thought) = pending_thought.take() {
                        messages.push(Message {
                            role: Role::Assistant,
                            content: thought,
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                        });
                    }
                    pending_thought = Some(text.clone());
                }
                ReActStep::Action(call) => {
                    let content = pending_thought.take().unwrap_or_default();
                    messages.push(Message {
                        role: Role::Assistant,
                        content,
                        tool_call_id: None,
                        tool_calls: vec![call.clone()],
                    });
                    pending_tool_calls.push_back(call.clone());
                }
                ReActStep::Observation(value) => {
                    let call = pending_tool_calls.pop_front().ok_or_else(|| {
                        WesichainError::Custom(
                            GraphError::InvalidToolCallResponse(
                                "observation without action".to_string(),
                            )
                            .to_string(),
                        )
                    })?;
                    messages.push(Message {
                        role: Role::Tool,
                        content: value.to_string(),
                        tool_call_id: Some(call.id),
                        tool_calls: Vec::new(),
                    });
                }
                ReActStep::FinalAnswer(text) => {
                    if let Some(thought) = pending_thought.take() {
                        messages.push(Message {
                            role: Role::Assistant,
                            content: thought,
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                        });
                    }
                    messages.push(Message {
                        role: Role::Assistant,
                        content: text.clone(),
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                    });
                }
                ReActStep::Error(text) => {
                    if let Some(thought) = pending_thought.take() {
                        messages.push(Message {
                            role: Role::Assistant,
                            content: thought,
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                        });
                    }
                    messages.push(Message {
                        role: Role::Assistant,
                        content: text.clone(),
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                    });
                }
            }
        }

        if let Some(thought) = pending_thought.take() {
            messages.push(Message {
                role: Role::Assistant,
                content: thought,
                tool_call_id: None,
                tool_calls: Vec::new(),
            });
        }

        if !pending_tool_calls.is_empty() {
            return Err(WesichainError::Custom(
                GraphError::InvalidToolCallResponse("tool calls missing observations".to_string())
                    .to_string(),
            ));
        }

        Ok(messages)
    }
}

impl ReActAgentNodeBuilder {
    pub fn llm(mut self, llm: Arc<dyn ToolCallingLlm>) -> Self {
        self.llm = Some(llm);
        self
    }

    pub fn tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools = tools;
        self
    }

    pub fn prompt(mut self, prompt: PromptTemplate) -> Self {
        self.prompt = prompt;
        self
    }

    pub fn max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    pub fn tool_failure_policy(mut self, tool_failure_policy: ToolFailurePolicy) -> Self {
        self.tool_failure_policy = tool_failure_policy;
        self
    }

    pub fn build(self) -> Result<ReActAgentNode, GraphError> {
        let llm = self
            .llm
            .ok_or_else(|| GraphError::InvalidToolCallResponse("missing llm".to_string()))?;
        let mut tools = HashMap::new();
        for tool in self.tools {
            let name = tool.name().to_string();
            if tools.contains_key(&name) {
                return Err(GraphError::DuplicateToolName(name));
            }
            tools.insert(name, tool);
        }
        let mut tool_specs: Vec<ToolSpec> = tools
            .iter()
            .map(|(name, tool)| ToolSpec {
                name: name.clone(),
                description: tool.description().to_string(),
                parameters: tool.schema(),
            })
            .collect();
        tool_specs.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(ReActAgentNode {
            llm,
            tools,
            tool_specs,
            prompt: self.prompt,
            max_iterations: self.max_iterations,
            tool_failure_policy: self.tool_failure_policy,
        })
    }
}

#[async_trait::async_trait]
impl<S> GraphNode<S> for ReActAgentNode
where
    S: StateSchema<Update = S> + ScratchpadState + HasUserInput + HasFinalOutput,
{
    async fn invoke_with_context(
        &self,
        input: GraphState<S>,
        context: &GraphContext,
    ) -> Result<StateUpdate<S>, WesichainError> {
        let mut data = input.data;
        data.ensure_scratchpad();

        let mut remaining = self
            .max_iterations
            .saturating_sub(data.iteration_count() as usize);
        if let Some(remaining_steps) = context.remaining_steps {
            remaining = remaining.min(remaining_steps);
        }

        if remaining == 0 {
            return Ok(StateUpdate::new(data));
        }

        let mut last_content: Option<String> = None;

        for _ in 0..remaining {
            let messages = self.build_messages(&data)?;
            let response = self
                .llm
                .invoke(LlmRequest {
                    model: String::new(),
                    messages,
                    tools: self.tool_specs.clone(),
                })
                .await?;
            let LlmResponse {
                content,
                tool_calls,
            } = response;
            last_content = Some(content.clone());
            data.increment_iteration();

            if tool_calls.is_empty() {
                data.scratchpad_mut()
                    .push(ReActStep::FinalAnswer(content.clone()));
                data.set_final_output(content);
                return Ok(StateUpdate::new(data));
            }

            if !content.is_empty() {
                data.scratchpad_mut().push(ReActStep::Thought(content));
            }

            for call in tool_calls {
                data.scratchpad_mut().push(ReActStep::Action(call.clone()));
                let tool = match self.tools.get(&call.name) {
                    Some(tool) => tool,
                    None => {
                        let error = GraphError::InvalidToolCallResponse(format!(
                            "unknown tool: {}",
                            call.name
                        ));
                        data.scratchpad_mut()
                            .push(ReActStep::Error(error.to_string()));
                        if let Some(observer) = &context.observer {
                            observer.on_error(&context.node_id, &error).await;
                        }
                        return Err(WesichainError::Custom(error.to_string()));
                    }
                };
                if let Some(observer) = &context.observer {
                    observer
                        .on_tool_call(&context.node_id, &call.name, &call.args)
                        .await;
                }
                match tool.invoke(call.args.clone()).await {
                    Ok(result) => {
                        data.scratchpad_mut()
                            .push(ReActStep::Observation(result.clone()));
                        if let Some(observer) = &context.observer {
                            observer
                                .on_tool_result(&context.node_id, &call.name, &result)
                                .await;
                        }
                    }
                    Err(err) => {
                        let reason = err.to_string();
                        match self.tool_failure_policy {
                            ToolFailurePolicy::FailFast => {
                                let error = GraphError::ToolCallFailed(call.name.clone(), reason);
                                data.scratchpad_mut()
                                    .push(ReActStep::Error(error.to_string()));
                                if let Some(observer) = &context.observer {
                                    observer.on_error(&context.node_id, &error).await;
                                }
                                return Err(WesichainError::Custom(error.to_string()));
                            }
                            ToolFailurePolicy::AppendErrorAndContinue => {
                                let message = format!("[TOOL ERROR] {}: {}", call.name, reason);
                                let value = Value::String(message);
                                data.scratchpad_mut()
                                    .push(ReActStep::Observation(value.clone()));
                                if let Some(observer) = &context.observer {
                                    observer
                                        .on_tool_result(&context.node_id, &call.name, &value)
                                        .await;
                                }
                            }
                        }
                    }
                }
            }
        }

        if data.final_output().is_none() {
            if let Some(content) = last_content {
                data.scratchpad_mut()
                    .push(ReActStep::FinalAnswer(content.clone()));
                data.set_final_output(content);
            }
        }

        Ok(StateUpdate::new(data))
    }
}
