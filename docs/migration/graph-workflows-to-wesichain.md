# Graph Workflows to Wesichain Migration Guide

This guide maps common Python graph-workflow concepts to Wesichain `wesichain-graph` APIs. The focus is stateful graphs, conditional routing, and persistence.

## Concept Mapping

| Python graph workflow concept | Wesichain |
| --- | --- |
| `StateGraph` style builder | `GraphBuilder` |
| `add_node` | `add_node` |
| `add_edge` | `add_edge` |
| `add_conditional_edges` | `add_conditional_edge` |
| `compile()` | `build()` |
| `app.invoke(state)` | `graph.invoke_graph(GraphState::new(state))` |
| `END` | No outgoing edge (graph terminates) |

`START` and `END` constants are available in Wesichain for graph-style wiring, while runtime termination still occurs naturally when there is no outgoing edge.

## State and Updates

Python graph runtimes usually use typed state with reducers. In Wesichain, implement `StateSchema::merge` to control update application.

```rust
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct AgentState {
    messages: Vec<String>,
    answer: Option<String>,
}

impl StateSchema for AgentState {
    fn merge(current: &Self, update: Self) -> Self {
        let mut messages = current.messages.clone();
        messages.extend(update.messages);
        let answer = if update.answer.is_some() {
            update.answer
        } else {
            current.answer.clone()
        };
        Self { messages, answer }
    }
}
```

## Building and Invoking a Graph

Python-style pseudocode:

```python
graph = StateGraph(State)
graph.add_node("agent", agent)
graph.set_entry_point("agent")
app = graph.compile()
app.invoke({"messages": []})
```

Wesichain:

```rust
let graph = GraphBuilder::new()
    .add_node("agent", Agent)
    .set_entry("agent")
    .build();

let out = graph
    .invoke_graph(GraphState::new(AgentState::default()))
    .await?;
```

## Conditional Routing

Python-style pseudocode:

```python
graph.add_conditional_edges("agent", route)
```

Wesichain:

```rust
let graph = GraphBuilder::new()
    .add_node("agent", Agent)
    .add_node("tools", ToolNode::new(tools))
    .add_conditional_edge("agent", |state| {
        if state.data.tool_calls.is_empty() {
            "final".to_string()
        } else {
            "tools".to_string()
        }
    })
    .add_edge("tools", "agent")
    .set_entry("agent")
    .build();
```

## ReAct Agent Pattern

Wesichain provides a dedicated `ReActAgentNode` for tool-using agent loops. This is the recommended way to build ReAct-style agents in production graph workflows.

```rust
use std::sync::Arc;

use wesichain_core::{HasFinalOutput, HasUserInput, ScratchpadState, ToolCallingLlm};
use wesichain_graph::{GraphBuilder, GraphState, ReActAgentNode, StateSchema};

// AppState implements:
// StateSchema + ScratchpadState + HasUserInput + HasFinalOutput
let llm: Arc<dyn ToolCallingLlm> = Arc::new(my_llm);

let node = ReActAgentNode::builder()
    .llm(llm)
    .tools(vec![Arc::new(CalculatorTool), Arc::new(SearchTool)])
    .max_iterations(12)
    .build()?;

let graph = GraphBuilder::new()
    .add_node("agent", node)
    .set_entry("agent")
    .build();

let state = GraphState::new(AppState::from_input("Investigate incident 423 and summarize."));
let out = graph.invoke_graph(state).await?;
println!("{:?}", out.data);
```

Runnable examples:

- `cargo run -p wesichain-graph --example react_agent`
- `cargo run -p wesichain-graph --example persistent_conversation`

## Checkpointing and Resume

Wesichain uses `Checkpointer` implementations and stores step/node metadata for thread-level persistence.

```rust
let checkpointer = InMemoryCheckpointer::default();
let graph = GraphBuilder::new()
    .add_node("agent", Agent)
    .set_entry("agent")
    .with_checkpointer(checkpointer.clone(), "thread-1")
    .build();

let out = graph
    .invoke_graph(GraphState::new(AgentState::default()))
    .await?;
let checkpoint = checkpointer.load("thread-1").await?.expect("checkpoint");
let resumed = graph.invoke_graph(checkpoint.state).await?;
```

## Interrupts (Human-in-the-Loop)

Wesichain can pause before or after specific nodes via static interrupt lists:

```rust
let graph = GraphBuilder::new()
    .add_node("prepare", Prepare)
    .add_node("review", Review)
    .add_edge("prepare", "review")
    .set_entry("prepare")
    .with_interrupt_before(["review"])
    .build();

match graph.invoke_graph(GraphState::new(ReviewState::default())).await {
    Err(GraphError::Interrupted) => {
        /* inspect checkpoint and resume */
    }
    _ => {}
}
```
