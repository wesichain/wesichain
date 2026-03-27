---
name: wesichain-tools
description: |
  Use when building custom tools for agents, using built-in file/HTTP/search tools,
  or setting up ToolBundle for agent toolkits. Use for implementing Tool trait,
  creating file operation tools, web search integration, or bundling multiple tools.
triggers:
  - custom tool
  - tool trait
  - file tools
  - http tools
  - search tools
  - ToolBundle
  - agent tools
  - tool implementation
  - tavily
  - bash exec
  - glob tool
  - grep tool
---

## When to Use

- **Implementing custom tools** for agents to extend capabilities
- **Using built-in tools** (file operations, HTTP, search)
- **Setting up agent toolkits** with ToolBundle
- **Integrating web search** via Tavily API
- **Executing shell commands** from agents (feature "exec")

## Quick Start

```rust
use async_trait::async_trait;
use serde_json::json;
use wesichain_core::{Tool, ToolError};
use wesichain_tools::ToolBundle;

// Quick setup with all default tools
let tools = ToolBundle::all_default();

// Or coding-focused toolkit
let coding_tools = ToolBundle::coding_tools();
```

## Key Patterns

### Pattern 1: Implementing a Custom Tool

The most common pattern - create a tool by implementing the `Tool` trait.

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wesichain_core::{Tool, ToolError};

// 1. Define input schema using structs
#[derive(Debug, Serialize, Deserialize)]
struct CalculatorInput {
    operation: String,  // "add", "subtract", "multiply", "divide"
    a: f64,
    b: f64,
}

// 2. Implement the Tool trait
#[derive(Debug, Clone)]
pub struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Perform basic arithmetic operations (add, subtract, multiply, divide)"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                    "description": "The arithmetic operation to perform"
                },
                "a": {
                    "type": "number",
                    "description": "First operand"
                },
                "b": {
                    "type": "number",
                    "description": "Second operand"
                }
            },
            "required": ["operation", "a", "b"]
        })
    }

    async fn invoke(&self, input: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        // 3. Parse input with proper error handling
        let input: CalculatorInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("Invalid input: {}", e)))?;

        // 4. Execute the tool logic
        let result = match input.operation.as_str() {
            "add" => input.a + input.b,
            "subtract" => input.a - input.b,
            "multiply" => input.a * input.b,
            "divide" => {
                if input.b == 0.0 {
                    return Err(ToolError::Execution("Division by zero".into()));
                }
                input.a / input.b
            }
            _ => return Err(ToolError::InvalidInput(
                format!("Unknown operation: {}", input.operation)
            )),
        };

        // 5. Return JSON Value
        Ok(json!({ "result": result }))
    }
}
```

### Pattern 2: Using Built-in File Tools

File operations are commonly needed for coding agents.

```rust
use wesichain_tools::{ReadFileTool, WriteFileTool, EditFileTool, GlobTool, GrepTool};

// Read file contents
let read_tool = ReadFileTool::new();
let content = read_tool.invoke(json!({
    "path": "/path/to/file.rs"
})).await?;

// Write new file
let write_tool = WriteFileTool::new();
write_tool.invoke(json!({
    "path": "/path/to/new_file.rs",
    "content": "fn main() {}"
})).await?;

// Edit existing file (find and replace)
let edit_tool = EditFileTool::new();
edit_tool.invoke(json!({
    "path": "/path/to/file.rs",
    "old_string": "fn old_name()",
    "new_string": "fn new_name()"
})).await?;

// Search for files by pattern
let glob_tool = GlobTool::new();
let files = glob_tool.invoke(json!({
    "pattern": "**/*.rs"
})).await?;

// Search file contents
let grep_tool = GrepTool::new();
let matches = grep_tool.invoke(json!({
    "pattern": "TODO|FIXME",
    "path": "."
})).await?;
```

### Pattern 3: Using ToolBundle for Quick Setup

ToolBundle provides pre-configured tool collections.

```rust
use wesichain_tools::ToolBundle;

// Basic bundle: HTTP + filesystem tools
let basic_tools = ToolBundle::all_default();

// Full coding toolkit: all file tools + HTTP + search
let coding_tools = ToolBundle::coding_tools();

// Convert to Vec<Box<dyn Tool>> for agent
let tool_vec: Vec<Box<dyn Tool>> = coding_tools.into_tools();

// Register with agent
let agent = ReActAgent::builder()
    .tools(tool_vec)
    .build();
```

**Available bundles:**

| Bundle | Tools Included | Use Case |
|--------|---------------|----------|
| `all_default()` | ReadFile, WriteFile, EditFile, Glob, Grep, HttpGet, HttpPost | General purpose agents |
| `coding_tools()` | All above + TavilySearch (if "search" feature enabled) | Coding/research agents |

### Pattern 4: HTTP Tools for API Calls

Make HTTP requests from agents.

```rust
use wesichain_tools::{HttpGetTool, HttpPostTool};

// GET request
let http_get = HttpGetTool::new();
let response = http_get.invoke(json!({
    "url": "https://api.example.com/data",
    "headers": {
        "Authorization": "Bearer token123"
    }
})).await?;

// POST request with JSON body
let http_post = HttpPostTool::new();
let response = http_post.invoke(json!({
    "url": "https://api.example.com/users",
    "headers": {
        "Content-Type": "application/json"
    },
    "body": {
        "name": "John",
        "email": "john@example.com"
    }
})).await?;
```

### Pattern 5: Search Tools for Web Retrieval

Enable web search capabilities (requires "search" feature).

```rust
use wesichain_tools::TavilySearchTool;

// Setup Tavily search (requires API key)
let search_tool = TavilySearchTool::new(
    "tvly-your-api-key".to_string()
);

// Search the web
let results = search_tool.invoke(json!({
    "query": "Rust async programming best practices",
    "max_results": 5
})).await?;

// Results include: title, url, content snippets
```

**Setup:** Set `TAVILY_API_KEY` environment variable or pass key directly.

## Golden Rules

1. **ALWAYS return `serde_json::Value`** - Tools must return JSON, never raw strings or custom types
2. **Use `#[async_trait]`** - All tool implementations must be async
3. **Validate inputs explicitly** - Return `ToolError::InvalidInput` for malformed input
4. **Include complete JSON Schema** - The `schema()` method must return valid JSON Schema
5. **Handle errors gracefully** - Return `ToolError::Execution` for runtime failures
6. **Cloneable tools** - Derive `Clone` for tools that will be shared across agents

## Common Mistakes

| Mistake | Problem | Fix |
|---------|---------|-----|
| Returning raw strings | Agent can't parse result | Always wrap in `json!({"result": ...})` |
| Missing `async_trait` | Won't compile with async trait | Add `#[async_trait]` before `impl Tool` |
| Incomplete schema | LLM generates invalid inputs | Include all properties and required fields |
| Panicking on errors | Crashes the agent | Return `ToolError` variants instead |
| Blocking in `invoke()` | Freezes async runtime | Use `tokio::task::spawn_blocking` for sync operations |
| Using `anyhow` errors | Loses error context | Use `ToolError` or `thiserror` enums |

## Resources

- Crate: `wesichain-tools`
- Core trait: `wesichain_core::Tool`
- Features: `exec` (BashExecTool), `search` (TavilySearchTool)
- Examples: `examples/tools_example.rs`
