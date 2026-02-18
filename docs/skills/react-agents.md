# Wesichain ReAct Agents

Build reasoning and acting agents that iteratively decide which tools to use, execute them, and reflect on results until reaching a final answer.

---

## Quick Reference

### Key Crates

```toml
[dependencies]
wesichain-core = "0.1"
wesichain-agent = "0.1"
wesichain-llm = "0.1"
wesichain-graph = "0.1"
```

### ReAct Loop

```
User Input
    |
    v
+---+------------------+
| LLM decides          |
| - Call tool?         |
| - Final answer?      |
+---+------------------+
    |
    +---> Tool Call? ---> Execute Tool ---> Update State ---+
    |                                                        |
    +--- Final Answer? -------------------------------------->
```

### Required State Traits

```rust
use wesichain_core::{StateSchema, HasToolCalls, HasUserInput, HasFinalOutput};

#[derive(StateSchema, Clone, Debug)]
struct AgentState {
    user_input: String,
    tool_calls: Vec<ToolCall>,
    tool_results: Vec<ToolResult>,
    final_output: Option<String>,
    iteration_count: u32,
}

impl HasToolCalls for AgentState {
    fn tool_calls(&self) -> &[ToolCall] { &self.tool_calls }
    fn set_tool_calls(&mut self, calls: Vec<ToolCall>) { self.tool_calls = calls; }
    fn add_tool_result(&mut self, result: ToolResult) { self.tool_results.push(result); }
}

impl HasUserInput for AgentState {
    fn user_input(&self) -> &str { &self.user_input }
}

impl HasFinalOutput for AgentState {
    fn final_output(&self) -> Option<&str> { self.final_output.as_deref() }
    fn set_final_output(&mut self, output: String) { self.final_output = Some(output); }
}
```

---

## Code Patterns

### Pattern 1: Basic ReAct Agent

Use `ReActGraphBuilder` for quick setup with sensible defaults.

```rust
use wesichain_agent::{ReActGraphBuilder, ReActState};
use wesichain_llm::OpenAIClient;

#[tokio::main]
async fn main() -> Result<()> {
    let llm = OpenAIClient::from_env()?;
    
    let agent = ReActGraphBuilder::new(llm)
        .with_tool(CalculatorTool)
        .with_tool(WeatherTool)
        .max_iterations(10)
        .build()?;
    
    let result = agent.run("What is 25 * 4 plus the temperature in Paris?").await?;
    println!("{}", result);
    
    Ok(())
}
```

### Pattern 2: Custom Graph Agent (Full Control)

Build your own graph for complete control over the ReAct loop.

```rust
use wesichain_graph::{GraphBuilder, Node, Edge};
use wesichain_agent::{AgentNode, ToolExecutorNode, ShouldContinueEdge};

#[tokio::main]
async fn main() -> Result<()> {
    let llm = OpenAIClient::from_env()?;
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(CalculatorTool),
        Box::new(SearchTool),
    ];
    
    let mut graph = GraphBuilder::<AgentState>::new();
    
    // Add nodes
    graph.add_node("agent", AgentNode::new(llm.clone(), &tools))?;
    graph.add_node("tools", ToolExecutorNode::new(tools))?;
    
    // Add edges with conditional routing
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
    let state = AgentState::new("Calculate fibonacci(10)");
    let result = compiled.run(state).await?;
    
    Ok(())
}
```

### Pattern 3: Multi-Tool Agent with Error Handling

Handle tool execution errors gracefully within the agent loop.

```rust
use wesichain_agent::{ReActGraphBuilder, ToolResult, ToolError};

#[derive(Clone)]
struct RobustCalculator;

#[async_trait]
impl Tool for RobustCalculator {
    fn name(&self) -> &str { "calculator" }
    
    async fn execute(&self, args: &str) -> Result<ToolResult, ToolError> {
        // Parse and validate arguments
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

// Build agent with error-handling tools
let agent = ReActGraphBuilder::new(llm)
    .with_tool(RobustCalculator)
    .with_tool(SearchTool::new().with_retry(3))
    .with_tool(APIClient::new().with_timeout(Duration::from_secs(10)))
    .on_tool_error(|err, state| {
        tracing::warn!("Tool failed: {}", err);
        // Continue with error as observation
        ToolResult::error(err.to_string())
    })
    .build()?;
```

### Pattern 4: Agent with Checkpointing (Resumable)

Enable resumable workflows for long-running or interruptible agents.

```rust
use wesichain_graph::{FileCheckpointer, CheckpointConfig};
use wesichain_agent::ReActGraphBuilder;

#[tokio::main]
async fn main() -> Result<()> {
    let checkpointer = FileCheckpointer::new("./checkpoints")
        .with_save_interval(Duration::from_secs(30));
    
    let agent = ReActGraphBuilder::new(llm)
        .with_tool(WebSearchTool)
        .with_tool(DocumentAnalyzer)
        .with_checkpointer(checkpointer)
        .build()?;
    
    // First run - may be interrupted
    let state = AgentState::new("Analyze 100 documents for compliance issues");
    let result = agent.run_with_checkpoint(state, "batch-analysis-001").await?;
    
    // Later - resume from checkpoint if interrupted
    if result.is_interrupted() {
        let resumed = agent.resume_from_checkpoint("batch-analysis-001").await?;
        println!("Resumed and completed: {}", resumed.final_output()?);
    }
    
    Ok(())
}

// Manual checkpoint control
async fn controlled_execution(agent: &Agent) -> Result<()> {
    let mut stream = agent.run_stream(state).await?;
    
    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::ToolCall { name, .. } if name == "expensive_operation" => {
                // Force checkpoint before expensive operation
                agent.checkpoint_now().await?;
            }
            AgentEvent::FinalAnswer(answer) => return Ok(answer),
            _ => continue,
        }
    }
    
    Ok(())
}
```

---

## Vibe Coding Prompts

### Prompt 1: Calculator Agent

```text
Create a ReAct agent that can perform multi-step mathematical calculations.
The agent should:
- Use a calculator tool for basic arithmetic (+, -, *, /, ^)
- Break down complex expressions into steps
- Show its work in the final answer
- Handle division by zero and other math errors gracefully

Example query: "Calculate (15 * 23 + 47) / 8 - 5^2"
```

### Prompt 2: Research Agent

```text
Build a research agent that searches the web and synthesizes information.
The agent should:
- Use web search to find current information
- Visit multiple sources for verification
- Cite sources in its final answer
- Ask clarifying questions if the query is ambiguous
- Limit itself to 5 search queries maximum

Example query: "What are the latest developments in fusion energy in 2025?"
```

### Prompt 3: RAG + Tools Agent

```text
Create a hybrid agent that combines document retrieval with external tools.
The agent should:
- First search a vector database for relevant documents
- Use retrieved context to answer if sufficient
- Fall back to web search if context is insufficient
- Use calculator for any numerical analysis
- Cite both documents and web sources appropriately

Example query: "What was our Q3 revenue and how does it compare to industry average?"
```

### Prompt 4: Multi-Agent Workflow

```text
Design a multi-agent system where specialized agents collaborate:
- Planner Agent: Breaks down complex tasks into subtasks
- Research Agent: Gathers information for each subtask
- Critic Agent: Reviews and validates findings
- Synthesizer Agent: Combines outputs into coherent final answer

Use message passing between agents with a shared state store.

Example query: "Write a comprehensive report on the environmental impact of electric vehicles"
```

---

## Common Errors

### State does not implement HasToolCalls

```
error[E0277]: the trait bound `MyState: HasToolCalls` is not satisfied
```

**Cause:** Your state struct is missing the `HasToolCalls` trait implementation.

**Fix:**
```rust
impl HasToolCalls for MyState {
    fn tool_calls(&self) -> &[ToolCall] { &self.tool_calls }
    fn set_tool_calls(&mut self, calls: Vec<ToolCall>) { self.tool_calls = calls; }
    fn add_tool_result(&mut self, result: ToolResult) { self.tool_results.push(result); }
}
```

### max iterations exceeded

```
Error: AgentError::MaxIterationsExceeded(10)
```

**Cause:** Agent looped more than the configured maximum without reaching a final answer.

**Fix:**
```rust
// Increase limit for complex tasks
ReActGraphBuilder::new(llm)
    .max_iterations(25)  // Default is 10
    .build()?;

// Or add a stopping condition
graph.add_conditional_edge("agent", |state: &AgentState| {
    if state.iteration_count > 5 && state.has_partial_answer() {
        return "end";
    }
    "continue"
}, vec![("continue", "tools"), ("end", "__end__")])?;
```

### tool not found

```
Error: ToolError::NotFound("calculatr")
```

**Cause:** LLM generated a tool name that doesn't match any registered tool (often a typo).

**Fix:**
```rust
// Provide clear tool descriptions to help LLM choose correctly
#[derive(Tool)]
#[tool(description = "A calculator for mathematical expressions. Use for any math operations.")]
struct Calculator;

// Or add fuzzy matching
impl Tool for MyToolSet {
    fn get_tool(&self, name: &str) -> Option<&dyn Tool> {
        // Try exact match first
        if let Some(tool) = self.tools.get(name) {
            return Some(tool.as_ref());
        }
        // Try fuzzy match
        self.tools.values()
            .find(|t| t.name().starts_with(name) || name.starts_with(t.name()))
            .map(|t| t.as_ref())
    }
}
```

### invalid tool arguments

```
Error: ToolError::InvalidArguments("missing field `expression`")
```

**Cause:** LLM provided malformed or missing arguments to the tool.

**Fix:**
```rust
#[derive(Tool)]
#[tool(description = "Calculator tool")]
struct Calculator;

#[derive(Deserialize)]
struct CalculatorArgs {
    #[serde(alias = "expr", alias = "equation")]  // Accept multiple field names
    expression: String,
}

#[async_trait]
impl Tool for Calculator {
    async fn execute(&self, args: &str) -> Result<ToolResult, ToolError> {
        // Provide helpful error message back to LLM
        let parsed: CalculatorArgs = match serde_json::from_str(args) {
            Ok(a) => a,
            Err(e) => {
                return Ok(ToolResult::error(format!(
                    "Invalid arguments. Expected JSON with 'expression' field. Error: {}",
                    e
                )));
            }
        };
        // ... execute
    }
}
```

### checkpoint serialization failed

```
Error: CheckpointError::SerializationFailed
```

**Cause:** State contains a type that cannot be serialized (e.g., raw socket, non-Serialize field).

**Fix:**
```rust
#[derive(StateSchema, Clone, Debug)]
struct AgentState {
    user_input: String,
    tool_calls: Vec<ToolCall>,
    // Mark non-serializable fields with #[serde(skip)]
    #[serde(skip)]
    runtime_cache: Arc<RwLock<HashMap<String, String>>>,
}

// Or implement custom serialization
impl Serialize for AgentState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AgentState", 3)?;
        state.serialize_field("user_input", &self.user_input)?;
        state.serialize_field("tool_calls", &self.tool_calls)?;
        // Skip runtime_cache
        state.end()
    }
}
```

---

## Best Practices

1. **Always set max_iterations** - Prevents infinite loops from runaway agents. Start with 10-15 for most tasks.

2. **Use specific tool descriptions** - Help the LLM choose correctly by describing when to use each tool and what arguments it expects.

3. **Implement graceful degradation** - When tools fail, return error messages as observations so the LLM can retry or work around the issue.

4. **Checkpoint long-running agents** - For tasks that may take minutes or hours, enable checkpointing to resume after interruptions.

5. **Limit context window** - Trim or summarize old tool results to prevent exceeding the LLM's context limit in long conversations.

6. **Validate tool outputs** - Sanitize tool results before sending to the LLM to prevent prompt injection from external sources.

7. **Use structured output for final answers** - When the agent output needs to be consumed programmatically, request JSON or a specific format in the system prompt.

---

## See Also

- [Graph Execution](graph-execution.md) - Building stateful execution graphs
- [Tool Definition](tool-definition.md) - Creating custom tools
- [LLM Integration](llm-integration.md) - Configuring LLM providers
- [Checkpointing](checkpointing.md) - State persistence patterns
