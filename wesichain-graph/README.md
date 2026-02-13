# Wesichain Graph

Stateful graph execution engine for building complex agents and workflows.

## Features

### State Management
Inspect and modify the graph state at runtime using thread IDs.

#### Get State
Retrieve the current state snapshot for a given thread.

```rust
let state = graph.get_state("thread_1").await?;
if let Some(s) = state {
    println!("Current state: {:?}", s.data);
}
```

#### Update State
Update the state of a thread, effectively "time traveling" or correcting the workflow.

```rust
graph.update_state(
    "thread_1", 
    MyState { key: "new_value".into() }, 
    Some("user_override".to_string())
).await?;
```
