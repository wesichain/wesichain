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
  - "wesichain-agent"
  - "max_iterations"
---

## When to Use

Use wesichain-react when you need to:
- Build agents that can use tools to solve multi-step problems
- Implement reasoning loops where the LLM decides what action to take next
- Create resumable workflows that can survive interruptions
- Combine multiple tools (calculator, search, APIs) in a single agent

## Quick Start

```rust
use wesichain_agent::{ReActGraphBuilder, ReActState};
use wesichain_llm::OpenAIClient;

let llm = OpenAIClient::from_env()?;

let agent = ReActGraphBuilder::new(llm)
    .with_tool(CalculatorTool)
    .with_tool(WeatherTool)
    .max_iterations(10)
    .build()?;

let result = agent.run("What is 25 * 4 plus the temperature in Paris?").await?;
println!("{}", result);
```

## Key Patterns

### Pattern 1: Basic ReAct Agent

```rust
use wesichain_agent::ReActGraphBuilder;

let agent = ReActGraphBuilder::new(llm)
    .with_tool(CalculatorTool)
    .with_tool(WeatherTool)
    .max_iterations(10)
    .build()?;

let result = agent.run("Your query here").await?;
```

### Pattern 2: Custom Graph Agent

```rust
use wesichain_graph::{GraphBuilder, Node, Edge};
use wesichain_agent::{AgentNode, ToolExecutorNode, ShouldContinueEdge};

let mut graph = GraphBuilder::<AgentState>::new();

// Add nodes
graph.add_node("agent", AgentNode::new(llm.clone(), &tools))?;
graph.add_node("tools", ToolExecutorNode::new(tools))?;

// Add conditional edges
graph.add_conditional_edge(
    "agent",
    ShouldContinueEdge::new(),
    vec![
        ("continue", "tools"),
        ("end", "__end__"),
    ],
)?;
graph.add_edge("tools", "agent")?;

let compiled = graph.build()?;
let result = compiled.run(state).await?;
```

### Pattern 3: Agent with Error Handling

```rust
use wesichain_agent::{ToolResult, ToolError};

#[async_trait]
impl Tool for RobustCalculator {
    fn name(&self) -> &str { "calculator" }
    
    async fn execute(&self, args: &str) -> Result<ToolResult, ToolError> {
        // Parse and validate
        let expr: Expr = match args.parse() {
            Ok(e) => e,
            Err(_) => return Ok(ToolResult::error("Invalid expression format")),
        };
        
        // Execute with timeout
        match tokio::time::timeout(
            Duration::from_secs(5),
            self.evaluate(expr)
        ).await {
            Ok(Ok(result)) => Ok(ToolResult::success(result.to_string())),
            Ok(Err(e)) => Ok(ToolResult::error(format!("Math error: {}", e))),
            Err(_) => Ok(ToolResult::error("Calculation timed out")),
        }
    }
}
```

### Pattern 4: Agent with Checkpointing

```rust
use wesichain_graph::{FileCheckpointer, CheckpointConfig};
use wesichain_agent::ReActGraphBuilder;

let checkpointer = FileCheckpointer::new("./checkpoints")
    .with_save_interval(Duration::from_secs(30));

let agent = ReActGraphBuilder::new(llm)
    .with_tool(WebSearchTool)
    .with_tool(DocumentAnalyzer)
    .with_checkpointer(checkpointer)
    .build()?;

// Run with checkpoint ID
let result = agent.run_with_checkpoint(state, "task-001").await?;

// Resume later if interrupted
if result.is_interrupted() {
    let resumed = agent.resume_from_checkpoint("task-001").await?;
}
```

## Golden Rules

1. **Always set max_iterations** - Prevents infinite loops; start with 10-15 for most tasks
2. **Use specific tool descriptions** - Help the LLM choose correctly by describing when to use each tool
3. **Implement graceful degradation** - Return error messages as observations so the LLM can retry
4. **Checkpoint long-running agents** - Enable resumability for tasks that may take minutes or hours
5. **Validate tool outputs** - Sanitize results before sending to LLM to prevent prompt injection

## Common Mistakes

- **State missing HasToolCalls trait** - Your state must implement `HasToolCalls`, `HasUserInput`, and `HasFinalOutput`
- **Max iterations exceeded** - Complex tasks need higher limits; increase `max_iterations` or add stopping conditions
- **Tool name typos** - LLM generates "calculatr" instead of "calculator"; provide clear descriptions and consider fuzzy matching
- **Invalid tool arguments** - LLM provides malformed JSON; accept multiple field names with serde aliases and provide helpful errors
- **Non-serializable state fields** - Raw sockets or locks break checkpointing; mark with `#[serde(skip)]` or use Arc

## Resources

- Full guide: `/Users/bene/Documents/bene/python/rechain/wesichain/.worktrees/ai-skills-docs/docs/skills/react-agents.md`
- Key crates: `wesichain-agent`, `wesichain-graph`
- Required traits: `HasToolCalls`, `HasUserInput`, `HasFinalOutput`, `StateSchema`
- Checkpointing: `FileCheckpointer`, `CheckpointConfig`
