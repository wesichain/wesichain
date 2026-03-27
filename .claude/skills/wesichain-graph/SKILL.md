---
name: wesichain-graph
description: |
  Build agent workflows with GraphBuilder, ReAct patterns, state management,
  conditional edges, checkpointing, and interrupts. Use for agent orchestration,
  human-in-the-loop, and fault-tolerant LLM workflows in Rust.
triggers:
  - "graph"
  - "GraphBuilder"
  - "ReAct"
  - "conditional edge"
  - "checkpointer"
  - "interrupt"
  - "wesichain-graph"
  - "ToolNode"
  - "HasToolCalls"
  - "StateSchema"
  - "human-in-the-loop"
---

## When to Use

Use wesichain-graph when you need to:
- Build complex agent workflows with conditional routing between nodes
- Create ReAct agents with LLM reasoning and tool execution loops
- Implement human-in-the-loop with interrupts for approval/review
- Add fault tolerance with checkpointing and resume capability
- Compose multi-step LLM pipelines with typed state management
- Execute tools in parallel with state-based orchestration

## Quick Start

```rust
use wesichain_graph::{GraphBuilder, GraphState, StateSchema, StateUpdate, START, END};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, StateSchema, Default)]
struct MyState {
    input: String,
    output: Option<String>,
}

let graph = GraphBuilder::new()
    .add_node("process", ProcessNode)
    .add_node("finalize", FinalizeNode)
    .add_edge(START, "process")
    .add_edge("process", "finalize")
    .add_edge("finalize", END)
    .build();

let state = GraphState::new(MyState::default());
let result = graph.invoke_graph(state).await?;
```

## Key Patterns

### Pattern 1: Linear Graph with GraphBuilder

```rust
use wesichain_graph::{GraphBuilder, GraphState, StateSchema, StateUpdate, START, END};

#[derive(Debug, Clone, Serialize, Deserialize, StateSchema, Default)]
struct PipelineState {
    query: String,
    context: Vec<String>,
    answer: Option<String>,
}

let graph = GraphBuilder::new()
    .add_node("retrieve", RetrieveNode)
    .add_node("generate", GenerateNode)
    .add_node("format", FormatNode)
    .add_edge(START, "retrieve")
    .add_edge("retrieve", "generate")
    .add_edge("generate", "format")
    .add_edge("format", END)
    .set_entry("retrieve")
    .build();

let state = GraphState::new(PipelineState {
    query: "What is Rust?".into(),
    ..Default::default()
});
let final_state = graph.invoke_graph(state).await?;
```

### Pattern 2: ReAct Agent with ReActGraphBuilder

```rust
use wesichain_graph::{
    ReActGraphBuilder, ScratchpadState, HasUserInput, HasFinalOutput
};
use wesichain_core::Tool;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, StateSchema, Default)]
struct AgentState {
    input: String,
    scratchpad: Vec<ReActStep>,
    final_output: Option<String>,
}

impl ScratchpadState for AgentState {
    fn scratchpad(&self) -> &[ReActStep] { &self.scratchpad }
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> { &mut self.scratchpad }
    fn ensure_scratchpad(&mut self) {}
    fn increment_iteration(&mut self) {}
}

impl HasUserInput for AgentState {
    fn user_input(&self) -> &str { &self.input }
}

impl HasFinalOutput for AgentState {
    fn final_output(&self) -> Option<&str> { self.final_output.as_deref() }
    fn set_final_output(&mut self, output: String) { self.final_output = Some(output); }
}

let tools: Vec<Arc<dyn Tool>> = vec![
    Arc::new(SearchTool),
    Arc::new(CalculatorTool),
];

let graph = ReActGraphBuilder::new()
    .llm(llm)
    .tools(tools)
    .build::<AgentState>()?;

let state = GraphState::new(AgentState {
    input: "What is 25 * 48?".into(),
    ..Default::default()
});
let result = graph.invoke_graph(state).await?;
```

### Pattern 3: Conditional Edges for Routing

```rust
use wesichain_graph::GraphBuilder;

let graph = GraphBuilder::new()
    .add_node("classify", ClassifyIntent)
    .add_node("search", SearchHandler)
    .add_node("calculate", CalculatorHandler)
    .add_node("chat", ChatHandler)
    .add_edge(START, "classify")
    .add_conditional_edge("classify", |state: &GraphState<MyState>| {
        match state.data.intent {
            Intent::Search => vec!["search".to_string()],
            Intent::Calculate => vec!["calculate".to_string()],
            Intent::Chat => vec!["chat".to_string()],
        }
    })
    .add_edge("search", END)
    .add_edge("calculate", END)
    .add_edge("chat", END)
    .set_entry("classify")
    .build();
```

### Pattern 4: Checkpointing for Fault Tolerance

```rust
use wesichain_graph::{FileCheckpointer, InMemoryCheckpointer};
use std::path::PathBuf;

// File-based persistence
let checkpointer = FileCheckpointer::new(PathBuf::from("./checkpoints"));

let graph = GraphBuilder::new()
    .add_node("step1", Step1)
    .add_node("step2", Step2)
    .add_edge(START, "step1")
    .add_edge("step1", "step2")
    .add_edge("step2", END)
    .with_checkpointer(checkpointer, "thread-123")
    .build();

// Resume from last checkpoint
let state = GraphState::new(MyState::default());
let result = graph.invoke_graph_with_options(
    state,
    ExecutionOptions {
        auto_resume: true,
        ..Default::default()
    }
).await?;

// Or manually resume
if let Some(checkpoint) = graph.get_state("thread-123").await? {
    let result = graph.resume(checkpoint, ExecutionOptions::default()).await?;
}
```

### Pattern 5: Interrupts for Human-in-the-Loop

```rust
use wesichain_graph::{ExecutionConfig, ExecutionOptions};

let graph = GraphBuilder::new()
    .add_node("draft", DraftNode)
    .add_node("review", ReviewNode)
    .add_node("send", SendNode)
    .add_edge(START, "draft")
    .add_edge("draft", "review")
    .add_edge("review", "send")
    .add_edge("send", END)
    .with_interrupt_before(["send"])  // Pause before sending
    .with_interrupt_after(["draft"]) // Pause after drafting
    .build();

// First run - will interrupt at draft completion
let state = GraphState::new(MyState::default());
let result = graph.invoke_graph(state).await;

// Handle the interrupt
match result {
    Err(GraphError::Interrupted) => {
        // Show UI for human review
        // Update state if needed
        // Resume execution
        let checkpoint = graph.get_state("thread-123").await?.unwrap();
        graph.resume(checkpoint, ExecutionOptions::default()).await?;
    }
    Ok(state) => println!("Completed: {:?}", state),
    Err(e) => println!("Error: {}", e),
}
```

### Pattern 6: ToolNode with HasToolCalls

```rust
use wesichain_graph::{ToolNode, HasToolCalls};
use wesichain_llm::{Message, ToolCall};

#[derive(Debug, Clone, StateSchema, Default)]
struct ToolState {
    messages: Vec<Message>,
    pending_calls: Vec<ToolCall>,
}

impl HasToolCalls for ToolState {
    fn tool_calls(&self) -> &Vec<ToolCall> {
        &self.pending_calls
    }

    fn push_tool_result(&mut self, message: Message) {
        self.messages.push(message);
    }
}

let tools = vec![
    Arc::new(SearchTool::default()),
    Arc::new(CalculatorTool::default()),
];

let graph = GraphBuilder::new()
    .add_node("agent", AgentNode)
    .add_node("tools", ToolNode::new(tools))
    .add_edge(START, "agent")
    .add_conditional_edge("agent", |state: &GraphState<ToolState>| {
        if state.data.pending_calls.is_empty() {
            vec![END.to_string()]
        } else {
            vec!["tools".to_string()]
        }
    })
    .add_edge("tools", "agent")
    .set_entry("agent")
    .build();
```

### Pattern 7: Custom State Reducers

```rust
use wesichain_graph::{StateReducer, Append, Override, Union};
use wesichain_core::state::Reducer;

#[derive(Debug, Clone, StateSchema)]
struct ChatState {
    #[reducer(Override)]
    current_query: String,
    
    #[reducer(Append)]
    messages: Vec<Message>,
    
    #[reducer(Union)]
    tags: HashSet<String>,
    
    #[reducer(AddCounter)]
    token_count: u64,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            current_query: String::new(),
            messages: Vec::new(),
            tags: HashSet::new(),
            token_count: 0,
        }
    }
}
```

## Golden Rules

1. **State MUST derive StateSchema** - Always use `#[derive(StateSchema)]` with serde traits
2. **Use START and END constants** - Never hardcode "__start" or "__end" strings
3. **Implement HasToolCalls for ToolNode** - Required trait for generic tool execution
4. **Use ReActGraphBuilder for agents** - Simpler than manual ReAct node construction
5. **Always set entry point** - Call `set_entry()` or the graph will panic on build
6. **Handle Interrupted errors** - Graph returns `GraphError::Interrupted` for human-in-the-loop
7. **Use with_checkpointer before build** - Checkpointer must be configured during graph construction
8. **Conditional edges return Vec<String>** - Can route to multiple targets for parallel execution

## Common Mistakes

- **Not deriving Default** - StateSchema requires Default for initialization
- **Using wrong tool node** - Use `ReActToolNode` for scratchpad states, `ToolNode` for HasToolCalls
- **Forgetting thread_id** - Required when using checkpointing; use consistent IDs for resuming
- **Ignoring StateUpdate** - Nodes must return `StateUpdate<S>`, not just the state data
- **Blocking in GraphNode::invoke_with_context** - Keep it async; spawn blocking work if needed
- **Missing error handling** - Graph can return `GraphError::Interrupted`, `GraphError::MaxStepsExceeded`, etc.
- **Not using auto_resume** - Without this flag, checkpoints are saved but not loaded automatically

## Resources

- Crate: `wesichain-graph`
- Key types: `GraphBuilder`, `ExecutableGraph`, `GraphState`, `StateUpdate`
- ReAct types: `ReActGraphBuilder`, `AgentNode`, `ReActToolNode`
- Checkpointing: `FileCheckpointer`, `InMemoryCheckpointer`, `HistoryCheckpointer`
- Tool integration: `ToolNode`, `HasToolCalls`, `ReActToolNode`
- State management: `StateSchema`, `StateReducer`, `Append`, `Override`, `Union`
- Constants: `START`, `END`
