# LangGraph to Wesichain Migration Guide

This guide maps LangGraph concepts to Wesichain's `wesichain-graph` APIs. The focus is on stateful graphs, conditional routing, and persistence.

## Concept Mapping

| LangGraph | Wesichain |
| --- | --- |
| `StateGraph` | `GraphBuilder` |
| `add_node` | `add_node` |
| `add_edge` | `add_edge` |
| `add_conditional_edges` | `add_conditional_edge` |
| `compile()` | `build()` |
| `app.invoke(state)` | `graph.invoke_graph(GraphState::new(state))` |
| `END` | No outgoing edge (graph terminates) |

`START` and `END` constants are available in Wesichain for LangGraph-style wiring, but the current runtime ends when there is no outgoing edge.

## State and Updates

LangGraph uses a typed state with reducers. In Wesichain, implement `StateSchema::merge` to control how updates are applied.

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

LangGraph:

```python
from langgraph.graph import StateGraph

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

LangGraph:

```python
graph.add_conditional_edges("agent", route)
```

Wesichain:

```rust
let graph = GraphBuilder::new()
    .add_node("agent", Agent)
    .add_node("tools", ToolNode::new(tools))
    .add_conditional_edge("agent", |state| {
        if state.data.tool_calls.is_empty() { "final".to_string() } else { "tools".to_string() }
    })
    .add_edge("tools", "agent")
    .set_entry("agent")
    .build();
```

## Checkpointing and Resume

LangGraph supports checkpointing for thread-level persistence. Wesichain uses `Checkpointer` implementations and stores step/node metadata.

```rust
let checkpointer = InMemoryCheckpointer::default();
let graph = GraphBuilder::new()
    .add_node("agent", Agent)
    .set_entry("agent")
    .with_checkpointer(checkpointer.clone(), "thread-1")
    .build();

let out = graph.invoke_graph(GraphState::new(AgentState::default())).await?;
let checkpoint = checkpointer.load("thread-1").await?.expect("checkpoint");
let resumed = graph.invoke_graph(checkpoint.state).await?;
```

## Interrupts (Human-in-the-Loop)

Wesichain can pause before or after nodes via static interrupt lists:

```rust
let graph = GraphBuilder::new()
    .add_node("prepare", Prepare)
    .add_node("review", Review)
    .add_edge("prepare", "review")
    .set_entry("prepare")
    .with_interrupt_before(["review"])
    .build();

match graph.invoke_graph(GraphState::new(ReviewState::default())).await {
    Err(GraphError::Interrupted) => { /* inspect checkpoint and resume */ }
    _ => {}
}
```
