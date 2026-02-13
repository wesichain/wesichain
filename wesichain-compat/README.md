# Wesichain Compat

A compatibility layer for Wesichain to support migration-oriented patterns and APIs.

## Features

- **Runnable Aliases**: compatibility aliases for incremental migration.
- **Batch Processing**: `batch` and `abatch` methods on `Runnable` for concurrent execution.
- **Binding**: `bind` method to attach arguments (e.g., tools) to runnables.
- **Chat Prompts**: `ChatPromptTemplate` for structured chat interactions.

## Usage

### Batch Processing

```rust
use wesichain_compat::RunnableExt;

let inputs = vec![input1, input2, input3];
let results = runnable.batch(inputs).await;
// results is Vec<Result<Output, Error>>
```

### Tool Binding & Macros

```rust
use wesichain_compat::Bindable;
use wesichain_macros::tool;
use serde_json::json;

#[tool(name = "calculator", description = "Adds two numbers")]
async fn add(a: i32, b: i32) -> Result<i32, String> {
    Ok(a + b)
}

// Bind tool to LLM
let tool = ADDTool;
let tool_spec = json!({ "tools": [{ 
    "name": tool.name(), 
    "description": tool.description(), 
    "parameters": tool.schema() 
}]});

llm.bind(tool_spec).unwrap();
```

### Chat Templates

```rust
use wesichain_prompt::{ChatPromptTemplate, MessagePromptTemplate};
use std::collections::HashMap;
use serde_json::json;

let prompt = ChatPromptTemplate::new(vec![
    MessagePromptTemplate::system("You are a helpful assistant."),
    MessagePromptTemplate::human("Hello, {{name}}!"),
]);

let mut vars = HashMap::new();
vars.insert("name".to_string(), json!("World"));

let messages = prompt.format_messages(&vars).unwrap();
```
