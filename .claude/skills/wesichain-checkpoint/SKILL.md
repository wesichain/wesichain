---
name: wesichain-checkpoint
description: |
  State persistence and checkpointing for Wesichain graphs. Provides SQLite, PostgreSQL,
  and Redis backends for saving and resuming graph execution. Use when building
  production agents that need persistence, resumability, or human-in-the-loop workflows.
triggers:
  - "checkpoint"
  - "persistence"
  - "save state"
  - "resume"
  - "SqliteCheckpointer"
  - "PostgresCheckpointer"
  - "RedisCheckpointer"
  - "thread_id"
---

## When to Use

Use wesichain-checkpoint when you need to:
- Save graph state between executions (persistent conversations)
- Resume interrupted workflows from where they left off
- Implement human-in-the-loop with review checkpoints
- Scale agents across multiple nodes with shared state
- Choose the right persistence backend for your deployment

## Quick Start

```rust
use wesichain_checkpoint_sqlite::SqliteCheckpointer;
use wesichain_graph::{GraphBuilder, Checkpointer};

// Create a SQLite checkpointer (best for development)
let checkpointer = SqliteCheckpointer::builder("sqlite::memory:")
    .max_connections(5)
    .build()
    .await?;

// Use with GraphBuilder for persistent execution
let graph = GraphBuilder::new()
    .add_node("agent", agent_node)
    .set_entry("agent")
    .with_checkpointer(checkpointer, "conversation-123")
    .build();

// State is automatically saved after each node execution
let result = graph.invoke(initial_state).await?;
```

## Key Patterns

### Pattern 1: SqliteCheckpointer for Development

Best for: Single-node deployments, local development, testing

```rust
use wesichain_checkpoint_sqlite::SqliteCheckpointer;

// In-memory (data lost on restart)
let checkpointer = SqliteCheckpointer::builder("sqlite::memory:")
    .max_connections(1)
    .build()
    .await?;

// File-based (persistent across restarts)
let checkpointer = SqliteCheckpointer::builder("sqlite:///path/to/app.db")
    .max_connections(5)
    .enable_projections(true)  // For state field indexing
    .build()
    .await?;
```

### Pattern 2: PostgresCheckpointer for Production

Best for: Multi-node deployments, production workloads, data durability

```rust
use wesichain_checkpoint_postgres::PostgresCheckpointer;

let checkpointer = PostgresCheckpointer::builder(
        std::env::var("DATABASE_URL")?
    )
    .max_connections(20)
    .min_connections(5)
    .enable_projections(true)
    .build()
    .await?;
```

### Pattern 3: RedisCheckpointer for High-Performance

Best for: High-throughput scenarios, ephemeral state, distributed caching

```rust
use wesichain_checkpoint_redis::RedisCheckpointer;

let checkpointer = RedisCheckpointer::new(
        "redis://localhost:6379",
        "my-app"  // namespace for key isolation
    )
    .await?
    .with_ttl(3600);  // Auto-expire after 1 hour
```

### Pattern 4: InMemoryCheckpointer for Testing

Best for: Unit tests, CI/CD, ephemeral environments

```rust
use wesichain_graph::InMemoryCheckpointer;

// Zero-setup, data only lives in process memory
let checkpointer = InMemoryCheckpointer::<MyState>::default();

// Use with Arc for sharing across tasks
let checkpointer = Arc::new(InMemoryCheckpointer::<MyState>::default());
```

### Pattern 5: Integrating with GraphBuilder

```rust
use wesichain_graph::{GraphBuilder, Checkpoint};
use std::sync::Arc;

let checkpointer = Arc::new(
    SqliteCheckpointer::builder("sqlite::memory:")
        .build()
        .await?
);

let graph = GraphBuilder::new()
    .add_node("agent", agent_node)
    .add_node("tools", tool_node)
    .set_entry("agent")
    .with_checkpointer(checkpointer.clone(), "thread-abc-123")
    .build();

// Execute - state automatically saved
let result = graph.invoke(initial_state).await?;

// Later, load the checkpoint to resume
let checkpoint: Option<Checkpoint<MyState>> = checkpointer
    .load("thread-abc-123")
    .await?;
```

### Pattern 6: Resuming from a Checkpoint

```rust
use wesichain_graph::{ExecutionOptions, Checkpoint};

// Load existing checkpoint
if let Some(checkpoint) = checkpointer.load("thread-abc-123").await? {
    // Create execution options from checkpoint
    let options = ExecutionOptions::from_checkpoint(checkpoint);
    
    // Resume execution from saved state
    let result = graph.invoke_with_options(state, options).await?;
}
```

## Backend Decision Matrix

| Backend | Best For | Durability | Scaling | Latency |
|---------|----------|------------|---------|---------|
| **InMemory** | Testing, CI/CD | None | Single-node only | Lowest |
| **SQLite** | Development, single-node | File-based | Single-node | Low |
| **PostgreSQL** | Production, multi-node | Full ACID | Horizontal | Medium |
| **Redis** | High-throughput, caching | Configurable TTL | Excellent | Lowest |

## Golden Rules

1. **Use SQLite for development** - Zero external dependencies, easy setup
2. **Use PostgreSQL for production** - ACID compliance, horizontal scaling support
3. **Use Redis for high-throughput** - When milliseconds matter, accept TTL tradeoffs
4. **Use InMemory for testing** - Fast, isolated, no cleanup needed
5. **Always use thread_id** - Required for checkpoint identification and loading
6. **Enable projections sparingly** - Only when querying specific state fields (adds overhead)
7. **Handle Option<Checkpoint>** - `load()` returns None if thread doesn't exist
8. **Clone checkpointer for reuse** - All checkpointer types are Clone + Send + Sync

## Common Mistakes

- **Forgetting to await build()** - Checkpointer builders return futures, must use `.await`
- **Using in-memory SQLite for production** - Data lost on restart; use file-based or PostgreSQL
- **Hardcoding thread_id** - Use UUIDs or user-specific identifiers for production
- **Not handling None from load()** - Always check if checkpoint exists before resuming
- **Sharing thread_id across different graphs** - Each graph needs unique thread identifiers
- **Enabling projections unnecessarily** - Only enable if querying state fields directly
- **Forgetting Arc wrapping** - Share checkpointer across tasks with `Arc::new()`

## Resources

- Crate: `wesichain-checkpoint-sqlite` | `wesichain-checkpoint-postgres` | `wesichain-checkpoint-redis`
- InMemory: `wesichain_graph::InMemoryCheckpointer`
- Key types: `Checkpoint<S>`, `Checkpointer<S>`, `ExecutionOptions`
- Example: `/Users/bene/Documents/bene/python/rechain/wesichain/wesichain-checkpoint-sqlite/examples/checkpoint_persistence.rs`
