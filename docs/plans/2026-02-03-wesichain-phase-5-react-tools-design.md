# Wesichain Phase 5 ReAct + Tool Calling Design

Date: 2026-02-03
Status: Draft

## Goals
- Make graph the canonical agent path via a ReActAgentNode.
- Add tool calling with OpenAI (async-openai) behind a feature flag.
- Provide structured scratchpad, safety, and observability suitable for production.

## Non-goals
- Streaming for ReActAgentNode.
- Conversation history or memory integration.
- Parallel tool execution.
- Additional provider integrations beyond OpenAI.

## Key Decisions
- ReActAgentNode is a public concrete type implementing Runnable.
- Tool trait, tool-calling types, and ToolCallingLlm live in wesichain-core.
- Scratchpad uses typed ReActStep with Observation(Value).
- Scratchpad lives in state S via ScratchpadState; auto-init if missing.
- Final answer writes to scratchpad and a dedicated state field (HasFinalOutput).
- User input comes from HasUserInput.
- Tool failures are configurable: FailFast or AppendErrorAndContinue.
- Malformed tool calls always fail fast.
- Sequential tool execution for MVP.
- OpenAI tool calling via async-openai in wesichain-llm with openai feature flag.
- System prompt contains behavior only; tool metadata goes in API tools param.
- Observer is graph-level and supplied via ExecutionOptions.

## Architecture Overview
ReActAgentNode lives in wesichain-graph and is invoked as a normal graph node. It uses a ToolCallingLlm trait from wesichain-core and executes tools provided as Vec<Arc<dyn Tool>>. The node builds structured chat messages each iteration (system + user + assistant/tool messages from scratchpad), sends tools via LlmRequest.tools, and loops until tool_calls is empty or a limit is reached.

Shared primitives (Message, Role, ToolSpec, ToolCall, LlmRequest, LlmResponse) move to wesichain-core and are re-exported by wesichain-llm to keep existing imports stable. ToolError is introduced in core and wrapped at the graph boundary.

## Core Interfaces (wesichain-core)

### Tool
```rust
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    async fn invoke(&self, args: Value) -> Result<Value, ToolError>;
}
```

ToolSpec keeps schema as Value with a minimal validation helper (root type must be object).

### ToolError
Domain-specific error type, wrapped into graph/agent errors at call sites.

### ToolCallingLlm
```rust
#[async_trait::async_trait]
pub trait ToolCallingLlm: Send + Sync + 'static {
    async fn invoke(&self, request: LlmRequest) -> Result<LlmResponse, WesichainError>;
}
```

### State Traits
```rust
pub trait ScratchpadState: Serialize + DeserializeOwned {
    fn scratchpad(&self) -> &Vec<ReActStep>;
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep>;
    fn iteration_count(&self) -> u32;
    fn increment_iteration(&mut self);
    fn ensure_scratchpad(&mut self) { /* auto-init if needed */ }
}

pub trait HasUserInput {
    fn user_input(&self) -> &str;
}

pub trait HasFinalOutput {
    fn final_output(&self) -> Option<&str>;
    fn set_final_output(&mut self, value: String);
}
```

### ReActStep
```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ReActStep {
    Thought(String),
    Action(ToolCall),
    Observation(Value),
    FinalAnswer(String),
    Error(String),
}
```

## ReActAgentNode (wesichain-graph)

### Construction
Builder accepts:
- llm: Arc<dyn ToolCallingLlm>
- tools: Vec<Arc<dyn Tool>>
- prompt: PromptTemplate (system-only) with default provided
- max_iterations (default, e.g. 12)
- tool_failure_policy (FailFast | AppendErrorAndContinue)

Builder validates tool name uniqueness; duplicates return GraphError (no panic).

### Message Construction
Each iteration builds messages from scratchpad:
- System message: rendered prompt (behavior only)
- User message: raw HasUserInput
- Assistant + Tool messages derived from ReActStep

Tool metadata is passed via LlmRequest.tools, not embedded in prompt.

### Iteration Limits
Effective limit = min(node.max_iterations, remaining_graph_budget). Remaining budget is provided by graph execution context. Scratchpad iteration count is persisted for resume safety.

### Error Policy
- ToolFailurePolicy::FailFast: append ReActStep::Error and return ToolCallFailed.
- AppendErrorAndContinue: append Observation(Value::String("[TOOL ERROR] ...")) and continue.
- Malformed tool calls: append ReActStep::Error and fail fast.

### StateUpdate
ReActAgentNode returns a single StateUpdate with full updated state after loop terminates (no sub-node checkpointing).

## Observability
Observer trait lives in wesichain-graph and is supplied via ExecutionOptions. Graph emits node start/end/error. ReActAgentNode emits tool-call and tool-result events.

## Provider Integration (wesichain-llm)
- OpenAI tool calling implemented via async-openai under feature flag `openai`.
- LLM/tool types are re-exported from core.

## Testing
- Core: ReActStep serde round-trip; ToolSpec schema validation; ScratchpadState iteration persistence.
- Graph: termination when tool_calls empty; sequential tool execution; failure policy behavior; malformed tool calls fail fast; iteration budget respects remaining graph steps.
- Observer: CollectingObserver verifies node/tool events.
- OpenAI: feature-gated compile/mapping tests (no API key required in CI).

## Examples
- Minimal ReAct graph using OpenAI tool calling (calculator tool).
- ToolFailurePolicy example (append error and continue).
- Observer example (console logging of tool calls).

## Rollout
- Move LLM/tool types to core; re-export from wesichain-llm.
- Add ToolCallingLlm in core; implement for OpenAI client behind feature flag.
- Add ReActAgentNode in wesichain-graph.
- Mark ToolCallingAgent in wesichain-agent as deprecated in favor of ReActAgentNode.
- Update README and add migration note.
