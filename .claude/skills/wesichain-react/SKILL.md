---
name: wesichain-react
description: |
  Build reasoning and acting (ReAct) agents that iteratively decide which tools
  to use, execute them, and reflect on results. Use for multi-step problem solving,
  tool-using agents, and autonomous workflows with checkpointing support.
triggers:
  - "react"
  - "agent"
  - "tool"
  - "reasoning"
  - "acting"
  - "checkpoint"
  - "wesichain-graph"
  - "ReActGraphBuilder"
---

## When to Use

Use wesichain-react when you need to:
- Build agents that can use tools to solve multi-step problems
- Implement reasoning loops where the LLM decides what action to take next
- Create resumable workflows that can survive interruptions
- Combine multiple tools (calculator, search, APIs) in a single agent

## Quick Start

```rust
use std::sync::Arc;
use wesichain_graph::ReActGraphBuilder;
use wesichain_core::{Tool, HasUserInput, HasFinalOutput, ScratchpadState};
use wesichain_graph::state::{StateSchema, GraphState};

// Define state with required traits
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct AgentState {
    input: String,
    scratchpad: Vec<wesichain_core::ReActStep>,
    final_output: Option<String>,
}

impl StateSchema for AgentState {
    type Update = Self;
    fn apply(_: &Self, update: Self) -> Self { update }
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
    fn scratchpad(&self) -> &Vec<wesichain_core::ReActStep> { &self.scratchpad }
    fn scratchpad_mut(&mut self) -> &mut Vec<wesichain_core::ReActStep> { &mut self.scratchpad }
}

// Build and run agent
let llm: Arc<dyn wesichain_core::ToolCallingLlm> = Arc::new(my_llm);

let graph = ReActGraphBuilder::new()
    .llm(llm)
    .tools(vec![Arc::new(CalculatorTool), Arc::new(SearchTool)])
    .build::<AgentState>()?;

let state = GraphState::new(AgentState::from_input("What is 25 * 4?"));
let result = graph.invoke_graph(state).await?;
println!("{:?}", result.data.final_output);
```

## Key Patterns

### Pattern 1: Basic ReAct Agent

```rust
use wesichain_graph::ReActGraphBuilder;
use std::sync::Arc;

let tools: Vec<Arc<dyn Tool>> = vec![
    Arc::new(CalculatorTool::default()),
    Arc::new(WeatherTool::default()),
];

let graph = ReActGraphBuilder::new()
    .llm(llm)
    .tools(tools)
    .build::<MyState>()?;

let result = graph.invoke_graph(GraphState::new(MyState::from_input("Query"))).await?;
```

### Pattern 2: Custom Graph with ToolNode

```rust
use wesichain_graph::{GraphBuilder, ToolNode, GraphState, StateUpdate, START, END};
use wesichain_graph::state::StateSchema;

// State implementing HasToolCalls
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
    fn push_tool_result(&mut self, message: Message) { self.messages.push(message); }
}

// Build graph
let tool_node = ToolNode::new(vec![Arc::new(CalculatorTool::default())]);

let graph = GraphBuilder::<CustomState>::new()
    .add_node("agent", my_llm_node)
    .add_node("tools", tool_node)
    .add_edge(START, "agent")
    .add_conditional_edge("agent", |state: &GraphState<CustomState>| {
        if state.data.final_answer.is_some() { vec![END.to_string()] }
        else if !state.data.tool_calls.is_empty() { vec!["tools".to_string()] }
        else { vec![END.to_string()] }
    })
    .add_edge("tools", "agent")
    .build()?;
```

### Pattern 3: Tool Implementation with Error Handling

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
                "expression": { "type": "string" }
            },
            "required": ["expression"]
        })
    }

    async fn invoke(&self, args: Value) -> Result<Value, ToolError> {
        let expr = args["expression"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("expression required".into()))?;

        match meval::eval_str(expr) {
            Ok(result) => Ok(serde_json::json!({ "result": result })),
            Err(e) => Err(ToolError::Execution(format!("Eval failed: {}", e))),
        }
    }
}
```

### Pattern 4: Agent with Checkpointing

```rust
use wesichain_graph::{InMemoryCheckpointer, Checkpoint};
use std::sync::Arc;

let checkpointer = Arc::new(InMemoryCheckpointer::<MyState>::default());

let graph = GraphBuilder::<MyState>::new()
    .add_node("agent", agent_node)
    .add_node("tools", tool_node)
    .add_edge(START, "agent")
    // ... conditional edges ...
    .with_checkpointer(checkpointer.clone(), "thread-123")
    .build()?;

// Execute
let result = graph.invoke_graph(GraphState::new(MyState::from_input("Task"))).await?;

// Resume later
if let Some(checkpoint) = checkpointer.load("thread-123").await? {
    let resumed = graph.invoke_graph(checkpoint.state).await?;
}
```

## Golden Rules

1. **State must implement all traits** - ReActGraphBuilder requires StateSchema + ScratchpadState + HasUserInput + HasFinalOutput
2. **Use Arc<dyn Tool> for tools** - ToolNode and ReActGraphBuilder expect tools wrapped in Arc
3. **Build is generic** - Call .build::<YourState>() with explicit state type
4. **Use invoke_graph() not run()** - ExecutableGraph uses invoke_graph() method
5. **Use START and END constants** - Don't hardcode "__start" and "__end"

## Common Mistakes

- **State missing ScratchpadState** - Required for ReActGraphBuilder; implement scratchpad() and scratchpad_mut()
- **Wrong builder method** - ReActGraphBuilder uses .llm() and .tools(), not .with_llm() or .with_tool()
- **Not using Arc for tools** - Tools must be Vec<Arc<dyn Tool>>, not Vec<Box<dyn Tool>>
- **Forgetting generic type** - build::<MyState>() is required; compiler can't infer the state type
- **Calling run() instead of invoke_graph()** - ExecutableGraph doesn't have run(), use invoke_graph()
- **State not serializable** - All state fields must implement Serialize + Deserialize for checkpointing

## Resources

- Full guide: `docs/skills/react-agents.md`
- Key crates: `wesichain-graph`, `wesichain-core`
- Types: `ReActGraphBuilder`, `ToolNode`, `GraphBuilder`, `ExecutableGraph`
- Traits: `Tool`, `StateSchema`, `ScratchpadState`, `HasUserInput`, `HasFinalOutput`, `HasToolCalls`
- Constants: `START`, `END`
