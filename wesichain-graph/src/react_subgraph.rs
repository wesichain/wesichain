use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use wesichain_core::{
    HasFinalOutput, HasUserInput, LlmRequest, LlmResponse, Message, ReActStep, Role, Runnable,
    ScratchpadState, Tool, ToolCall, ToolCallingLlm, ToolSpec, Value, WesichainError,
};
use wesichain_prompt::PromptTemplate;

use crate::config::ExecutionConfig;
use crate::error::GraphError;
use crate::graph::{ExecutableGraph, GraphBuilder, GraphContext, GraphNode};

use crate::state::{GraphState, StateSchema, StateUpdate};
use crate::{END, START};

const DEFAULT_SYSTEM_PROMPT: &str = "You are a helpful assistant. Use tools when helpful. If a tool is used, wait for the tool result before answering.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolFailurePolicy {
    #[default]
    FailFast,
    AppendErrorAndContinue,
}

// ── Context compression ───────────────────────────────────────────────────────

/// Strategy for compressing the message history when it grows too large.
#[async_trait::async_trait]
pub trait ContextCompressor: Send + Sync {
    /// Return `true` if the current message list should be compressed.
    fn should_compress(&self, messages: &[Message]) -> bool;
    /// Replace the message list with a shorter summarised version.
    async fn compress(&self, messages: Vec<Message>) -> Result<Vec<Message>, WesichainError>;
}

/// Compresses message history when the total character count exceeds `max_chars`.
///
/// Uses an LLM to produce a one-sentence summary that replaces the middle
/// messages (keeping the system prompt and the last user message intact).
pub struct TokenThresholdCompressor {
    max_chars: usize,
    llm: Arc<dyn Runnable<LlmRequest, LlmResponse>>,
}

impl TokenThresholdCompressor {
    /// `max_chars / 4` approximates the token count.
    pub fn new(max_chars: usize, llm: Arc<dyn Runnable<LlmRequest, LlmResponse>>) -> Self {
        Self { max_chars, llm }
    }
}

#[async_trait::async_trait]
impl ContextCompressor for TokenThresholdCompressor {
    fn should_compress(&self, messages: &[Message]) -> bool {
        let total: usize = messages.iter().map(|m| m.content.to_string().len()).sum();
        total > self.max_chars
    }

    async fn compress(&self, messages: Vec<Message>) -> Result<Vec<Message>, WesichainError> {
        // Keep system prompt (first) + last user message + compress everything in between.
        let history_text: String = messages[1..]
            .iter()
            .map(|m| format!("{:?}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let summary_req = LlmRequest {
            model: String::new(),
            messages: vec![
                Message {
                    role: Role::System,
                    content: "Summarise the following conversation in 2-3 sentences.".into(),
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                Message {
                    role: Role::User,
                    content: history_text.into(),
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            tools: vec![],
            temperature: Some(0.0),
            max_tokens: Some(256),
            stop_sequences: vec![],
        };

        let summary = self.llm.invoke(summary_req).await?.content;

        let mut compressed = Vec::new();
        // Keep system prompt if present
        if let Some(sys) = messages.first() {
            if matches!(sys.role, Role::System) {
                compressed.push(sys.clone());
            }
        }
        compressed.push(Message {
            role: Role::User,
            content: format!("[Context summary] {summary}").into(),
            tool_call_id: None,
            tool_calls: vec![],
        });
        // Keep the last user message if different from summary
        if let Some(last) = messages.last() {
            if matches!(last.role, Role::User) {
                compressed.push(last.clone());
            }
        }
        Ok(compressed)
    }
}

// ── AgentNode ─────────────────────────────────────────────────────────────────

/// Node that executes the LLM in the ReAct loop.
/// It inspects the state, builds messages, calls the LLM, and updates the scratchpad
/// with the Thought (if any) and Action (if tool calls generated).
pub struct AgentNode {
    llm: Arc<dyn ToolCallingLlm>,
    tools: Vec<ToolSpec>,
    prompt: PromptTemplate,
    context_compressor: Option<Arc<dyn ContextCompressor>>,
}

impl AgentNode {
    pub fn new(llm: Arc<dyn ToolCallingLlm>, tools: Vec<ToolSpec>, prompt: PromptTemplate) -> Self {
        Self { llm, tools, prompt, context_compressor: None }
    }

    pub fn with_context_compressor(mut self, compressor: Arc<dyn ContextCompressor>) -> Self {
        self.context_compressor = Some(compressor);
        self
    }

    fn build_messages_robust<S>(&self, state: &S) -> Result<Vec<Message>, WesichainError>
    where
        S: ScratchpadState + HasUserInput,
    {
        let mut messages = Vec::new();
        let prompt = self.prompt.render(&HashMap::new())?;
        messages.push(Message {
            role: Role::System,
            content: prompt.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        });
        messages.push(Message {
            role: Role::User,
            content: state.user_input().to_string().into(),
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
                            content: thought.into(),
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
                        content: content.into(),
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
                        content: value.to_string().into(),
                        tool_call_id: Some(call.id),
                        tool_calls: Vec::new(),
                    });
                }
                ReActStep::FinalAnswer(text) => {
                    if let Some(thought) = pending_thought.take() {
                        messages.push(Message {
                            role: Role::Assistant,
                            content: thought.into(),
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                        });
                    }
                    messages.push(Message {
                        role: Role::Assistant,
                        content: text.clone().into(),
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                    });
                }
                ReActStep::Error(text) => {
                    if let Some(thought) = pending_thought.take() {
                        messages.push(Message {
                            role: Role::Assistant,
                            content: thought.into(),
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                        });
                    }
                    messages.push(Message {
                        role: Role::Assistant,
                        content: text.clone().into(),
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                    });
                }
            }
        }

        if let Some(thought) = pending_thought.take() {
            messages.push(Message {
                role: Role::Assistant,
                content: thought.into(),
                tool_call_id: None,
                tool_calls: Vec::new(),
            });
        }

        Ok(messages)
    }
}

#[async_trait::async_trait]
impl<S> GraphNode<S> for AgentNode
where
    S: StateSchema<Update = S>
        + ScratchpadState
        + HasUserInput
        + HasFinalOutput
        + Default
        + Send
        + Sync,
{
    async fn invoke_with_context(
        &self,
        input: GraphState<S>,
        _context: &GraphContext,
    ) -> Result<StateUpdate<S>, WesichainError> {
        let mut data = input.data;
        data.ensure_scratchpad();

        // Build messages from current scratchpad history
        let mut messages = self.build_messages_robust(&data)?;

        // Apply context compression if configured and threshold is exceeded.
        if let Some(compressor) = &self.context_compressor {
            if compressor.should_compress(&messages) {
                messages = compressor.compress(messages).await?;
            }
        }

        let response = self
            .llm
            .invoke(LlmRequest {
                model: String::new(),
                messages,
                tools: self.tools.clone(),
                temperature: None,
                max_tokens: None,
                stop_sequences: vec![],
            })
            .await?;

        let LlmResponse {
            content,
            tool_calls,
            ..
        } = response;

        // Create delta for update
        let mut delta = S::default();
        delta.ensure_scratchpad();
        delta.increment_iteration(); // Assuming we track iteration count in State

        // Update scratchpad based on LLM output
        if tool_calls.is_empty() {
            // No tools -> Final Answer
            delta
                .scratchpad_mut()
                .push(ReActStep::FinalAnswer(content.clone()));
            delta.set_final_output(content);
        } else {
            // Tools requested -> Action
            if !content.is_empty() {
                delta.scratchpad_mut().push(ReActStep::Thought(content));
            }
            for call in tool_calls {
                delta.scratchpad_mut().push(ReActStep::Action(call));
            }
        }

        Ok(StateUpdate::new(delta))
    }
}

/// Node that executes tools based on pending Actions in the scratchpad.
/// It finds the last Action(s) that do not have a following Observation,
/// executes them, and appends the Observation.
/// Tool execution node specialized for ReAct-style agents.
///
/// Unlike [`ToolNode`](crate::ToolNode) which uses the [`HasToolCalls`](crate::HasToolCalls) trait,
/// `ReActToolNode` works with states implementing [`ScratchpadState`](wesichain_core::ScratchpadState).
/// It reads the latest tool call from the scratchpad, executes it, and pushes
/// the result back as a new [`ReActStep`](wesichain_core::ReActStep).
///
/// Use this when building agents via [`ReActGraphBuilder`].
/// For generic tool execution in non-ReAct workflows, use [`ToolNode`](crate::ToolNode).
pub struct ReActToolNode {
    tools: HashMap<String, Arc<dyn Tool>>,
    failure_policy: ToolFailurePolicy,
}

impl ReActToolNode {
    pub fn new(tools: HashMap<String, Arc<dyn Tool>>, failure_policy: ToolFailurePolicy) -> Self {
        Self {
            tools,
            failure_policy,
        }
    }
}

#[async_trait::async_trait]
impl<S> GraphNode<S> for ReActToolNode
where
    S: StateSchema<Update = S> + ScratchpadState + Default + Send + Sync,
{
    async fn invoke_with_context(
        &self,
        input: GraphState<S>,
        context: &GraphContext,
    ) -> Result<StateUpdate<S>, WesichainError> {
        let data = input.data;

        let mut actions_to_execute = Vec::new();
        let scratchpad = data.scratchpad();

        for step in scratchpad.iter().rev() {
            match step {
                ReActStep::Action(call) => actions_to_execute.push(call.clone()),
                ReActStep::Observation(_) => break,
                ReActStep::FinalAnswer(_) => break,
                ReActStep::Error(_) => break,
                ReActStep::Thought(_) => continue,
            }
        }
        actions_to_execute.reverse();

        if actions_to_execute.is_empty() {
            return Ok(StateUpdate::new(S::default()));
        }

        let mut delta = S::default();
        delta.ensure_scratchpad();

        let mut join_set = tokio::task::JoinSet::new();

        for (index, call) in actions_to_execute.into_iter().enumerate() {
            let tool = match self.tools.get(&call.name) {
                Some(tool) => tool.clone(),
                None => {
                    let error =
                        GraphError::InvalidToolCallResponse(format!("unknown tool: {}", call.name));
                    // We can't really fail fast easily in parallel without aborting all.
                    // For now, let's treat unknown tool as an immediate error result.
                    // But to respect sequential ordering, we just return error.
                    // Or we could let it return an Error observation for this specific tool?
                    // ReAct logic typically halts on error.
                    // Let's mimic original logic: return Err immediately.
                    // Wait, original logic returned Err.
                    // We'll spawn a task that returns Err.
                    join_set.spawn(async move {
                        (index, call, Err(WesichainError::Custom(error.to_string())))
                    });
                    continue;
                }
            };

            if let Some(observer) = &context.observer {
                observer
                    .on_tool_call(&context.node_id, &call.name, &call.args)
                    .await;
            }

            let node_id = context.node_id.clone();
            let observer = context.observer.clone();
            let _failure_policy = self.failure_policy;

            join_set.spawn(async move {
                let result = tool
                    .invoke(call.args.clone())
                    .await
                    .map_err(|e| WesichainError::Custom(e.to_string()));
                // Side effects like observer can happen here or after join.
                // Doing here is fine.
                if let Some(observer) = &observer {
                    match &result {
                        Ok(res) => observer.on_tool_result(&node_id, &call.name, res).await,
                        Err(_err) => {
                            // Assuming FailFast for observer notification, or we can notify error
                            // Actually the original code notified on error depending on policy.
                        }
                    }
                }
                (index, call, result)
            });
        }

        // Collect results
        let mut results = Vec::new();
        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(val) => results.push(val),
                Err(err) => {
                    return Err(WesichainError::Custom(format!("Tool task failed: {}", err)))
                }
            }
        }

        // Sort by index to maintain order
        results.sort_by_key(|(i, _, _)| *i);

        for (_, call, result) in results {
            match result {
                Ok(output) => {
                    delta.scratchpad_mut().push(ReActStep::Observation(output));
                }
                Err(err) => {
                    let reason = err.to_string();
                    match self.failure_policy {
                        ToolFailurePolicy::FailFast => {
                            let error = GraphError::ToolCallFailed(call.name.clone(), reason);
                            delta
                                .scratchpad_mut()
                                .push(ReActStep::Error(error.to_string()));
                            if let Some(observer) = &context.observer {
                                observer.on_error(&context.node_id, &error).await;
                            }
                            return Err(WesichainError::Custom(error.to_string()));
                        }
                        ToolFailurePolicy::AppendErrorAndContinue => {
                            let message = format!("[TOOL ERROR] {}: {}", call.name, reason);
                            let value = Value::String(message);
                            delta
                                .scratchpad_mut()
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

        Ok(StateUpdate::new(delta))
    }
}

pub struct ReActGraphBuilder {
    llm: Option<Arc<dyn ToolCallingLlm>>,
    tools: Vec<Arc<dyn Tool>>,
    prompt: PromptTemplate,
    tool_failure_policy: ToolFailurePolicy,
    context_compressor: Option<Arc<dyn ContextCompressor>>,
}

impl Default for ReActGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ReActGraphBuilder {
    pub fn new() -> Self {
        Self {
            llm: None,
            tools: Vec::new(),
            prompt: PromptTemplate::new(DEFAULT_SYSTEM_PROMPT.to_string()),
            tool_failure_policy: ToolFailurePolicy::FailFast,
            context_compressor: None,
        }
    }

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

    pub fn tool_failure_policy(mut self, policy: ToolFailurePolicy) -> Self {
        self.tool_failure_policy = policy;
        self
    }

    /// Attach a context compressor to the agent node.
    pub fn with_context_compressor(mut self, compressor: impl ContextCompressor + 'static) -> Self {
        self.context_compressor = Some(Arc::new(compressor));
        self
    }

    pub fn build<S>(self) -> Result<ExecutableGraph<S>, GraphError>
    where
        S: StateSchema<Update = S>
            + ScratchpadState
            + HasUserInput
            + HasFinalOutput
            + Default
            + Send
            + Sync,
    {
        let llm = self
            .llm
            .ok_or_else(|| GraphError::Checkpoint("Missing LLM".into()))?;

        let mut tool_map = HashMap::new();
        let mut tool_specs = Vec::new();

        for tool in &self.tools {
            if tool_map.contains_key(tool.name()) {
                return Err(GraphError::DuplicateToolName(tool.name().to_string()));
            }
            tool_map.insert(tool.name().to_string(), tool.clone());
            tool_specs.push(ToolSpec {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.schema(),
            });
        }

        let mut agent_node = AgentNode::new(llm, tool_specs, self.prompt);
        if let Some(compressor) = self.context_compressor {
            agent_node = agent_node.with_context_compressor(compressor);
        }
        let agent_node = agent_node;
        let tool_node = ReActToolNode::new(tool_map, self.tool_failure_policy);

        let builder = GraphBuilder::<S>::new()
            .with_default_config(ExecutionConfig {
                cycle_detection: false,
                ..Default::default()
            })
            .add_node("agent", agent_node)
            .add_node("tools", tool_node)
            .add_edge(START, "agent")
            .add_conditional_edge("agent", |state: &GraphState<S>| {
                // Check if last step in scratchpad is Action
                // If Action -> go to tools
                // If FinalAnswer -> go to END
                if let Some(step) = state.data.scratchpad().last() {
                    match step {
                        ReActStep::Action(_) => vec!["tools".to_string()],
                        _ => vec![END.to_string()],
                    }
                } else {
                    // Start or empty? Should not happen after agent.
                    // But if agent produced nothing, maybe end?
                    vec![END.to_string()]
                }
            })
            .add_edge("tools", "agent")
            .set_entry("agent");

        Ok(builder.build())
    }
}
