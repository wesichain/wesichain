---
name: wesichain-langsmith
description: |
  Add observability and tracing to Wesichain applications with LangSmith integration.
  Use for debugging LLM chains, monitoring production performance, and analyzing
  tool usage patterns.
triggers:
  - langsmith
  - tracing
  - observability
  - callback handler
  - graph observer
  - debug chain
  - monitor llm
---

## When to Use

Use wesichain-langsmith when you need to:

- **Debug failing chains**: See exactly what inputs/outputs flow through each step
- **Monitor production**: Track latency, token usage, and error rates
- **Analyze tool usage**: Understand which tools are called and with what arguments
- **Optimize prompts**: Compare different prompt versions side-by-side
- **Collaborate**: Share run traces with team members for review

### Integration Pattern Decision Tree

| Scenario | Use | Why |
|----------|-----|-----|
| Any `Runnable` (chains, agents) | **CallbackHandler** | Works universally |
| `Graph` workflows | **Observer** | Richer graph-specific data |
| Both chain + graph in same app | **Both** | Complete visibility |
| Need custom span timing | **Observer** | Fine-grained control |

## Quick Start

```rust
use wesichain_langsmith::{LangSmithConfig, LangSmithCallbackHandler};
use wesichain_core::Runnable;

#[tokio::main]
async fn main() {
    // 1. Configure LangSmith
    let config = LangSmithConfig::new()
        .with_api_key(std::env::var("LANGSMITH_API_KEY").unwrap())
        .with_project("my-production-app");
    
    // 2. Create handler
    let handler = LangSmithCallbackHandler::new(config);
    
    // 3. Wrap your chain
    let chain = my_chain.with_callbacks(handler);
    
    // 4. Run - traces appear in LangSmith UI
    let result = chain.invoke("Hello, world!").await.unwrap();
}
```

## Key Patterns

### Pattern 1: Basic CallbackHandler Setup

Use for any `Runnable` - chains, agents, or custom components.

```rust
use wesichain_langsmith::{LangSmithConfig, LangSmithCallbackHandler};
use wesichain_core::CallbackManager;

let config = LangSmithConfig::new()
    .with_api_key(api_key)
    .with_project("production");

let handler = LangSmithCallbackHandler::new(config);
let callback_manager = CallbackManager::new().with_handler(handler);

// Wrap your runnable
let traced_chain = chain.with_callbacks(callback_manager);
```

**Production tip**: Set `LANGSMITH_API_KEY` env var to avoid hardcoding secrets.

### Pattern 2: Graph-Specific Observer

Use for `Graph` workflows to get structured node execution data.

```rust
use wesichain_graph::{Graph, ExecutionOptions};
use wesichain_langsmith::LangSmithObserver;

let observer = LangSmithObserver::new(
    LangSmithConfig::new()
        .with_api_key(api_key)
        .with_project("agent-workflows")
);

let options = ExecutionOptions::new()
    .with_observer(observer);

let result = graph
    .invoke_with_options(input, options)
    .await
    .unwrap();
```

**Benefits over CallbackHandler**:
- Node-level granularity in traces
- Automatic edge traversal visualization
- State transitions shown in UI

### Pattern 3: Adding Metadata and Tags

Organize traces for filtering and analysis.

```rust
use wichain_langsmith::{RunMetadata, RunTags};

let metadata = RunMetadata::new()
    .with("user_id", "user_123")
    .with("session_id", "sess_456")
    .with("model_version", "gpt-4")
    .with("custom_metric", 42.0);

let tags = RunTags::new()
    .add("production")
    .add("v2.1")
    .add("high-priority");

let config = LangSmithConfig::new()
    .with_api_key(api_key)
    .with_default_metadata(metadata)
    .with_default_tags(tags);
```

**Use cases**:
- Filter by `user_id` to debug specific user issues
- Tag by version to compare releases
- Track `session_id` for conversation debugging

### Pattern 4: Sampling for Cost Control

Reduce trace volume in high-throughput scenarios.

```rust
use wesichain_langsmith::{ProbabilitySampler, SamplingConfig};

// Sample 10% of traces
let sampler = ProbabilitySampler::new(0.1);

let config = LangSmithConfig::new()
    .with_api_key(api_key)
    .with_sampler(sampler);
```

**When to sample**:
- High-volume production (>1000 calls/hour)
- Cost-sensitive operations
- Health check endpoints
- Load testing scenarios

**When NOT to sample**:
- Error investigations
- New feature rollouts
- Low-volume debugging

### Pattern 5: Viewing Traces in LangSmith UI

After running your code:

1. Go to [smith.langchain.com](https://smith.langchain.com)
2. Navigate to your project
3. Find traces by:
   - Time range
   - Tags/metadata filters
   - Search by input/output content
   - Error status

**Key views**:
- **Trace view**: Step-by-step execution flow
- **Compare**: Side-by-side run comparison
- **Metrics**: Latency, token usage trends
- **Playground**: Replay with modifications

## Golden Rules

1. **Never commit API keys** - Use `LANGSMITH_API_KEY` env var
2. **Use Observer for graphs** - Richer data than CallbackHandler
3. **Add metadata early** - Easier to debug later
4. **Sample in production** - Control costs at scale
5. **Tag by environment** - Separate dev/staging/prod traces
6. **Name projects clearly** - `myapp-production` not `project-1`
7. **Flush on shutdown** - Don't lose traces on SIGTERM

## Common Mistakes

### Mistake: Blocking on trace export
```rust
// BAD - blocks runtime
handler.flush().await;

// GOOD - spawn in background
tokio::spawn(async move {
    handler.flush().await;
});
```

### Mistake: No project separation
```rust
// BAD - all traces mixed
LangSmithConfig::new().with_api_key(key)

// GOOD - environment separation
LangSmithConfig::new()
    .with_api_key(key)
    .with_project(format!("myapp-{}", env::var("ENV").unwrap()))
```

### Mistake: Forgetting to flush on exit
```rust
// In your shutdown handler
let exporter = LangSmithExporter::new(config);
exporter.flush().await.expect("Failed to flush traces");
```

### Mistake: Tracing health checks
```rust
// BAD - traces every health check
if path == "/health" {
    return chain.invoke(HealthCheck).await;
}

// GOOD - skip tracing for health checks
let config = LangSmithConfig::new()
    .with_sampler(HealthCheckSampler::new());
```

## Resources

- **LangSmith Dashboard**: https://smith.langchain.com
- **API Docs**: https://docs.smith.langchain.com
- **Pricing**: https://www.langchain.com/pricing
- **Wesichain Examples**: `examples/langsmith_integration.rs`
