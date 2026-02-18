# Wesichain ReAct Agents

Build reasoning and acting (ReAct) agents that iteratively decide which tools to use, execute them, and reflect on results until reaching a final answer.

## Quick Reference

### Key Crates

```rust
use wesichain_graph::{ReActGraphBuilder, GraphBuilder, ToolNode, ExecutableGraph};
use wesichain_core::{Tool, ToolCall, ToolCallingLlm};
use wesichain_graph::state::{StateSchema, GraphState, StateUpdate};
use wesichain_graph::{START, END};
```

### ReAct Loop

```
User Input → LLM decides → [Tool Call] → Execute Tool → LLM decides → ... → Final Answer
```

### Required State Traits

```rust
use wesichain_graph::state::StateSchema;
use wesichain_core::{HasUserInput, HasFinalOutput, ScratchpadState, ReActStep};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct AgentState {
    input: String,
    scratchpad: Vec<ReActStep>,
    final_output: Option<String>,
}

impl StateSchema for AgentState {
    type Update = Self;
    fn apply(_current: &Self, update: Self) -> Self { update }
}

impl HasUserInput for AgentState {
    fn from_input(input: impl Into<String>) -> Self {
        Self { input: input.into(), ..Default::default() }
    }
    fn input(&self) -> &str { &self.input }
}

impl HasFinalOutput for AgentState {
    fn final_output(&self) -> Option<&str> { self.final_output.as_deref() }
}

impl ScratchpadState for AgentState {
    fn scratchpad(&self) -> &Vec<ReActStep> { &self.scratchpad }
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> { &mut self.scratchpad }
}
```

## Code Patterns

### Pattern 1: Basic ReAct Agent with ReActGraphBuilder

```rust
use std::sync::Arc;
use wesichain_graph::ReActGraphBuilder;
use wesichain_core::{HasUserInput, HasFinalOutput, ScratchpadState};
use wesichain_graph::state::{StateSchema, GraphState};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct MyState {
    input: String,
    scratchpad: Vec<wesichain_core::ReActStep>,
    final_output: Option<String>,
}

impl StateSchema for MyState {
    type Update = Self;
    fn apply(_: &Self, update: Self) -> Self { update }
}

impl HasUserInput for MyState {
    fn from_input(input: impl Into<String>) -> Self {
        Self { input: input.into(), ..Default::default() }
    }
    fn input(&self) -> &str { &self.input }
}

impl HasFinalOutput for MyState {
    fn final_output(&self) -> Option<&str> { self.final_output.as_deref() }
}

impl ScratchpadState for MyState {
    fn scratchpad(&self) -> &Vec<wesichain_core::ReActStep> { &self.scratchpad }
    fn scratchpad_mut(&mut self) -> &mut Vec<wesichain_core::ReActStep> { &mut self.scratchpad }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create LLM that supports tool calling
    let llm: Arc<dyn ToolCallingLlm> = Arc::new(
        wesichain_llm::OpenAiCompatibleClient::new()
            .base_url("http://localhost:11434/v1")
            .model("llama3.1")
            .build()?
    );

    // 2. Create tools
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(CalculatorTool::default()),
        Arc::new(SearchTool::default()),
    ];

    // 3. Build ReAct agent using ReActGraphBuilder
    let graph = ReActGraphBuilder::new()
        .llm(llm)
        .tools(tools)
        .build::<MyState>()?;

    // 4. Execute
    let state = GraphState::new(MyState::from_input(
        "What is 25 * 4?"
    ));

    let result = graph.invoke_graph(state).await?;
    println!("Answer: {:?}", result.data.final_output);

    Ok(())
}
```

### Pattern 2: Custom Graph Agent with ToolNode

```rust
use std::sync::Arc;
use wesichain_graph::{GraphBuilder, ToolNode, GraphState, StateUpdate, START, END};
use wesichain_graph::state::StateSchema;
use wesichain_core::{Runnable, LlmRequest, LlmResponse, ToolCall, Message, Role};
use wesichain_llm::ToolCallingLlm;

// State that tracks tool calls
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct CustomState {
    messages: Vec<Message>,
    tool_calls: Vec<ToolCall>,
    final_answer: Option<String>,
}

impl StateSchema for CustomState {
    type Update = Self;
    fn apply(_: &Self, update: Self) -> Self { update }
}

impl wesichain_graph::HasToolCalls for CustomState {
    fn tool_calls(&self) -> &Vec<ToolCall> { &self.tool_calls }
    fn push_tool_result(&mut self, message: Message) {
        self.messages.push(message);
    }
}

// Agent node that calls LLM
struct LlmNode {
    llm: Arc<dyn ToolCallingLlm>,
}

#[async_trait::async_trait]
impl Runnable<GraphState<CustomState>, StateUpdate<CustomState>> for LlmNode {
    async fn invoke(&self, input: GraphState<CustomState>) -> Result<StateUpdate<CustomState>, wesichain_core::WesichainError> {
        let request = LlmRequest {
            messages: input.data.messages.clone(),
            ..Default::default()
        };
        let response = self.llm.invoke(request).await?;

        let mut next = input.data.clone();
        if let Some(calls) = response.tool_calls {
            next.tool_calls = calls;
        } else {
            next.final_answer = Some(response.content);
        }

        Ok(StateUpdate::new(next))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm: Arc<dyn ToolCallingLlm> = Arc::new(/* your LLM */);

    let llm_node = LlmNode { llm: llm.clone() };
    let tool_node = ToolNode::new(vec![
        Arc::new(CalculatorTool::default())
    ]);

    // Build graph with conditional edges
    let graph = GraphBuilder::<CustomState>::new()
        .add_node("agent", llm_node)
        .add_node("tools", tool_node)
        .add_edge(START, "agent")
        .add_conditional_edge("agent", |state: &GraphState<CustomState>| {
            if state.data.final_answer.is_some() {
                vec![END.to_string()]
            } else if !state.data.tool_calls.is_empty() {
                vec!["tools".to_string()]
            } else {
                vec![END.to_string()]
            }
        })
        .add_edge("tools", "agent")
        .build()?;

    let state = GraphState::new(CustomState::default());
    let result = graph.invoke_graph(state).await?;

    Ok(())
}
```

### Pattern 3: Multi-Tool Agent with Error Handling

```rust
use wesichain_core::{Tool, ToolError};
use serde_json::Value;

#[derive(Default)]
pub struct CalculatorTool;

#[async_trait::async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str { "calculator" }

    fn description(&self) -> &str {
        "Evaluate mathematical expressions safely"
    }

    fn schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Math expression to evaluate"
                }
            },
            "required": ["expression"]
        })
    }

    async fn invoke(&self, args: Value) -> Result<Value, ToolError> {
        let expr = args["expression"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("expression required".into()))?;

        // Use meval for safe evaluation
        match meval::eval_str(expr) {
            Ok(result) => Ok(serde_json::json!({ "result": result })),
            Err(e) => Err(ToolError::Execution(format!("Eval failed: {}", e))),
        }
    }
}

// Build agent with multiple tools
let tools: Vec<Arc<dyn Tool>> = vec![
    Arc::new(CalculatorTool::default()),
    Arc::new(WeatherTool::default()),
    Arc::new(SearchTool::default()),
];

let graph = ReActGraphBuilder::new()
    .llm(llm)
    .tools(tools)
    .build::<MyState>()?;
```

### Pattern 4: Agent with Checkpointing (Resumable)

```rust
use wesichain_graph::{InMemoryCheckpointer, Checkpoint};
use wesichain_graph::state::StateSchema;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create checkpointer
    let checkpointer = Arc::new(InMemoryCheckpointer::<MyState>::default());

    // Build graph with checkpointer
    let graph = GraphBuilder::<MyState>::new()
        .add_node("agent", agent_node)
        .add_node("tools", tool_node)
        // ... edges ...
        .with_checkpointer(checkpointer.clone(), "thread-123")
        .build()?;

    // First execution
    let state = GraphState::new(MyState::from_input("Calculate 2+2"));
    let result = graph.invoke_graph(state).await?;

    // Later: Resume from checkpoint
    if let Some(checkpoint) = checkpointer.load("thread-123").await? {
        let resumed_state = checkpoint.state;
        let new_result = graph.invoke_graph(resumed_state).await?;
    }

    Ok(())
}
```

## Vibe Coding Prompts

### Prompt 1: Calculator Agent

"Create a Wesichain ReAct agent using ReActGraphBuilder that can solve math problems. Implement a Calculator tool using the Tool trait with meval for evaluation. Use Ollama as the LLM and handle tool errors gracefully."

### Prompt 2: Research Agent

"Build a research agent with ReActGraphBuilder that has web search and calculator tools. The agent should answer questions like 'What is the GDP of Japan divided by 1000?' by searching for GDP data and then calculating."

### Prompt 3: Custom Graph Agent

"Create a custom graph agent using GraphBuilder (not ReActGraphBuilder) with explicit nodes for 'llm' and 'tools'. Use HasToolCalls trait on state and implement conditional edges to route between nodes based on whether tool calls are present."

### Prompt 4: Multi-Agent System

"Build a supervisor agent using GraphBuilder that routes tasks to specialized agents (calculator, researcher, writer). Each agent is a separate graph. The supervisor uses conditional edges to route based on task type."

## Common Errors

### Error: "State does not implement ScratchpadState"

```
error[E0277]: the trait bound `MyState: ScratchpadState` is not satisfied
```

**Cause**: ReActGraphBuilder requires ScratchpadState for tracking thoughts/actions.

**Fix**: Implement all required traits:

```rust
impl ScratchpadState for MyState {
    fn scratchpad(&self) -> &Vec<ReActStep> { &self.scratchpad }
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> { &mut self.scratchpad }
}
```

### Error: "max iterations exceeded"

```
Error: Graph execution exceeded maximum iterations
```

**Cause**: Agent keeps looping without reaching final answer.

**Fix**: Add iteration limit in your state or use tool_failure_policy:

```rust
// Track iterations in state
#[derive(Clone, Debug)]
struct MyState {
    iteration_count: u32,
    // ... other fields
}

// Or check for infinite loops in conditional edges
.add_conditional_edge("agent", |state: &GraphState<MyState>| {
    if state.data.iteration_count > 10 {
        vec![END.to_string()]
    } else if has_tool_calls(state) {
        vec!["tools".to_string()]
    } else {
        vec![END.to_string()]
    }
})
```

### Error: "tool not found"

```
Error: Tool 'unknown_tool' not found in registry
```

**Fix**: Ensure tool name in schema matches what LLM generates:

```rust
fn name(&self) -> &str { "calculator" } // Must match LLM output

// In schema, be explicit:
fn schema(&self) -> Value {
    serde_json::json!({
        "name": "calculator", // Should match name()
        // ...
    })
}
```

### Error: "checkpoint serialization failed"

```
Error: Failed to serialize checkpoint: missing field
```

**Fix**: Ensure all state fields implement Serialize + Deserialize:

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct MyState {
    // All fields must be serializable
    input: String,
    tool_calls: Vec<ToolCall>, // ToolCall must also be serializable
}
```

### Error: "future cannot be sent between threads"

```
error[E0277]: `std::rc::Rc<MyTool>` cannot be sent between threads safely
```

**Fix**: Use Arc for tools and ensure all types are Send + Sync:

```rust
// Wrong
let tools: Vec<Rc<dyn Tool>> = vec![...];

// Correct
let tools: Vec<Arc<dyn Tool>> = vec![
    Arc::new(MyTool::default()),
];
```

## Best Practices

1. **Use ReActGraphBuilder for Standard Cases**: It handles the ReAct loop correctly with minimal boilerplate.

2. **State Must Derive Clone + Serialize + Deserialize**: Required for checkpointing and graph execution:
   ```rust
   #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
   struct MyState { ... }
   ```

3. **Tools Must Be Wrapped in Arc**: ToolNode and ReActToolNode expect `Arc<dyn Tool>`:
   ```rust
   let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(MyTool::default())];
   ```

4. **Use TypeState Pattern for Builders**: The build() method is generic over state type:
   ```rust
   let graph = ReActGraphBuilder::new()
       .llm(llm)
       .tools(tools)
       .build::<MyState>()?;
   ```

5. **Handle Tool Errors Gracefully**: Return ToolError variants, not panics:
   ```rust
   async fn invoke(&self, args: Value) -> Result<Value, ToolError> {
       if invalid {
           return Err(ToolError::InvalidInput("reason".into()));
       }
       // ...
   }
   ```

6. **Use START and END Constants**: Don't hardcode "__start" and "__end":
   ```rust
   use wesichain_graph::{START, END};
   .add_edge(START, "agent")
   .add_edge("agent", END)
   ```

7. **Checkpoint for Long-Running Tasks**: Use checkpointer to enable resumability:
   ```rust
   let checkpointer = Arc::new(InMemoryCheckpointer::<MyState>::default());
   let graph = GraphBuilder::new()
       .with_checkpointer(checkpointer, "thread-id")
       .build()?;
   ```

## See Also

- [Core Concepts](./core-concepts.md) - Runnable, Chain, Tool traits
- [RAG Pipelines](./rag-pipelines.md) - Document retrieval and context injection
- [Crates.io](https://crates.io/crates/wesichain-graph) - wesichain-graph crate
- [Examples](https://github.com/wesichain/wesichain/tree/main/examples) - Working agent examples
