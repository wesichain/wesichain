# Wesichain Core Concepts

Foundation patterns for building with Wesichain: Runnable trait, Chain composition, Tool trait, and LCEL-style pipelines.

## Quick Reference

### Core Traits

```rust
use wesichain_core::{Runnable, Chain, Tool};
use wesichain_prompt::PromptTemplate;
use wesichain_llm::{Llm, ChatMessage};
```

### Type Signatures

| Type | Signature | Description |
|------|-----------|-------------|
| `Runnable<I,O>` | `async fn invoke(&self, input: I) -> Result<O, Error>` | Core composable unit |
| `Chain<Head,Tail,Mid>` | `Head.then(Tail)` - sequential composition | Pipeline composition |
| `Tool` | `name()`, `description()`, `invoke(input)` | ReAct-compatible tool |

## Code Patterns

### Pattern 1: Basic Runnable Chain (LCEL-Style)

Build a complete LLM chain using the `.then()` combinator, similar to LangChain's LCEL syntax.

```rust
use wesichain_core::{Runnable, Chain};
use wesichain_prompt::PromptTemplate;
use wesichain_llm::{Llm, ChatMessage, DummyLlm};

// Define the components
let prompt = PromptTemplate::new(
    "You are a helpful assistant. Answer: {question}"
);

let llm = DummyLlm::new(); // Replace with OpenAI, Anthropic, etc.

let parser = |response: String| -> Result<String, Box<dyn std::error::Error>> {
    Ok(response.trim().to_string())
};

// Chain them together with .then()
let chain = prompt
    .then(llm)
    .then(parser);

// Execute the chain
let result = chain.invoke("What is Rust?".to_string()).await?;
println!("Result: {}", result);
```

### Pattern 2: Custom Tool Implementation

Implement the `Tool` trait to create ReAct-compatible tools for agents.

```rust
use wesichain_core::{Tool, ToolError};
use serde_json::{json, Value};
use async_trait::async_trait;

pub struct Calculator;

#[async_trait]
impl Tool for Calculator {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Perform basic arithmetic operations. Input: {\"operation\": \"add\", \"a\": 1, \"b\": 2}"
    }

    async fn invoke(&self,
        input: Value
    ) -> Result<Value, ToolError> {
        let operation = input.get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput(
                "Missing 'operation' field".to_string()
            ))?;

        let a = input.get("a")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ToolError::InvalidInput(
                "Missing 'a' field".to_string()
            ))?;

        let b = input.get("b")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ToolError::InvalidInput(
                "Missing 'b' field".to_string()
            ))?;

        let result = match operation {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Err(ToolError::InvalidInput(
                        "Division by zero".to_string()
                    ));
                }
                a / b
            }
            _ => return Err(ToolError::InvalidInput(
                format!("Unknown operation: {}", operation)
            )),
        };

        Ok(json!({ "result": result }))
    }
}

// Usage
let calc = Calculator;
let input = json!({
    "operation": "add",
    "a": 10,
    "b": 32
});
let output = calc.invoke(input).await?;
println!("10 + 32 = {}", output["result"]); // 42
```

### Pattern 3: Chain with Retries and Fallbacks

Build resilient chains that handle transient failures gracefully.

```rust
use wesichain_core::{Runnable, Chain};
use wesichain_llm::{Llm, DummyLlm, LlmError};

// Create primary and fallback LLMs
let primary_llm = DummyLlm::new();
let fallback_llm = DummyLlm::new(); // Different provider in production

// Chain with retry logic
let resilient_chain = primary_llm
    .with_retries(3) // Retry up to 3 times on failure
    .with_fallback(fallback_llm); // Use fallback if all retries fail

// The chain will:
// 1. Try primary_llm
// 2. Retry up to 3 times on transient errors
// 3. Fall back to fallback_llm if primary fails completely
let result = resilient_chain.invoke(
    "Generate a poem".to_string()
).await?;
```

### Pattern 4: Custom Runnable Implementation

Implement the `Runnable` trait directly for custom transformations.

```rust
use wesichain_core::{Runnable, RunnableError};
use async_trait::async_trait;

// Custom runnable that transforms input to uppercase
pub struct UppercaseTransformer;

#[async_trait]
impl Runnable<String, String> for UppercaseTransformer {
    async fn invoke(
        &self,
        input: String
    ) -> Result<String, RunnableError> {
        Ok(input.to_uppercase())
    }

    async fn stream(
        &self,
        input: String
    ) -> Result<Box<dyn Stream<Item = Result<String, RunnableError>> + Send>, RunnableError> {
        // For simple cases, just return the full result as a single stream item
        let result = input.to_uppercase();
        let stream = futures::stream::once(async move { Ok(result) });
        Ok(Box::new(stream))
    }
}

// Usage in a chain
let transformer = UppercaseTransformer;
let result = transformer.invoke("hello world".to_string()).await?;
assert_eq!(result, "HELLO WORLD");

// Chain with other runnables
let chain = UppercaseTransformer.then(DummyLlm::new());
```

## Vibe Coding Prompts

### Prompt 1: Basic LLM Chain

> "Create a Wesichain chain that takes a user question, formats it with a system prompt about being a helpful coding assistant, sends it to an LLM, and parses the response. Use PromptTemplate, DummyLlm, and a simple string parser."

### Prompt 2: Calculator Tool

> "Implement a Calculator tool for Wesichain that supports add, subtract, multiply, and divide operations. The tool should accept JSON input with 'operation', 'a', and 'b' fields and return JSON with a 'result' field. Handle division by zero and unknown operations with proper ToolError variants."

### Prompt 3: Resilient Chain

> "Build a resilient Wesichain chain that retries failed LLM calls 3 times before falling back to a secondary LLM provider. Use the with_retries and with_fallback methods on the Runnable trait."

## Common Errors

### Error: trait bound not satisfied

```
error[E0277]: the trait bound `MyComponent: Runnable<String, String>` is not satisfied
```

**Cause**: Your type doesn't implement the required `Runnable` trait.

**Fix**: Implement the `Runnable` trait with proper input/output types:

```rust
use async_trait::async_trait;
use wesichain_core::{Runnable, RunnableError};

#[async_trait]
impl Runnable<String, String> for MyComponent {
    async fn invoke(&self, input: String) -> Result<String, RunnableError> {
        // Implementation
        Ok(input)
    }
}
```

### Error: cannot borrow as mutable

```
error[E0596]: cannot borrow `self` as mutable, as it is not declared as mutable
```

**Cause**: Trying to mutate state in a non-mutable context.

**Fix**: Use interior mutability (Arc<Mutex<T>>) or restructure to avoid mutation:

```rust
use std::sync::{Arc, Mutex};

pub struct StatefulRunnable {
    counter: Arc<Mutex<i32>>,
}

#[async_trait]
impl Runnable<String, String> for StatefulRunnable {
    async fn invoke(&self,
        input: String
    ) -> Result<String, RunnableError> {
        let mut count = self.counter.lock().unwrap();
        *count += 1;
        Ok(format!("Count: {}, Input: {}", *count, input))
    }
}
```

### Error: future cannot be sent between threads

```
error: future cannot be sent between threads safely
```

**Cause**: Missing `Send` bounds on trait objects or streams.

**Fix**: Ensure all async trait methods return `Send` futures:

```rust
#[async_trait]
impl Runnable<String, String> for MyRunnable {
    // The macro handles Send bounds automatically
    async fn invoke(&self,
        input: String
    ) -> Result<String, RunnableError> {
        // Use tokio::spawn or other async operations safely
        tokio::task::yield_now().await;
        Ok(input)
    }
}
```

### Error: ToolError::InvalidInput

```
ToolError::InvalidInput("Missing required field: 'query'")
```

**Cause**: Tool received malformed or incomplete input JSON.

**Fix**: Validate input thoroughly and provide descriptive error messages:

```rust
async fn invoke(&self,
    input: Value
) -> Result<Value, ToolError> {
    let query = input.get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidInput(
            "Missing required field: 'query' (string)".to_string()
        ))?;

    // Validate query length
    if query.is_empty() {
        return Err(ToolError::InvalidInput(
            "Query cannot be empty".to_string()
        ));
    }

    // Proceed with valid input
    Ok(json!({ "result": "success" }))
}
```

### Error: missing field (prompt template)

```
Error: TemplateError("Missing variable: 'question'")
```

**Cause**: Prompt template expects variables that weren't provided in the input.

**Fix**: Ensure all template variables are provided:

```rust
let prompt = PromptTemplate::new(
    "Answer: {question} in the style of {persona}"
);

// Wrong - missing 'persona'
let result = prompt.invoke(json!({
    "question": "What is AI?"
})).await?;

// Correct - all variables provided
let result = prompt.invoke(json!({
    "question": "What is AI?",
    "persona": "a friendly teacher"
})).await?;
```

## See Also

- [RAG Pipelines](./rag-pipelines.md) - Build retrieval-augmented generation systems
- [ReAct Agents](./react-agents.md) - Create reasoning and acting agents
- [Examples](../examples/) - Complete working examples
