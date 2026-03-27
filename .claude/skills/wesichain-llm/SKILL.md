---
name: wesichain-llm
description: |
  LLM client abstractions for Wesichain: OpenAI-compatible client, provider-specific
  clients (OpenAI, DeepSeek, Google, Azure, Mistral, Ollama), tool calling support,
  streaming, and LlmRequest/LlmResponse types.
triggers:
  - "llm"
  - "openai"
  - "deepseek"
  - "google"
  - "azure"
  - "mistral"
  - "ollama"
  - "OpenAiClient"
  - "OpenAiCompatibleClient"
  - "LlmRequest"
  - "LlmResponse"
  - "ToolSpec"
  - "ToolCall"
---

## When to Use

Use wesichain-llm when you need to:
- Integrate LLM providers (OpenAI, DeepSeek, Google, Azure, Mistral, Ollama) into your application
- Make tool calling requests with structured ToolSpec definitions
- Stream LLM responses for real-time UX
- Use OpenAI-compatible endpoints for custom providers or self-hosted models
- Build provider-agnostic LLM components that work across multiple backends

## Quick Start

```rust
use wesichain_core::Runnable;
use wesichain_llm::{OpenAiClient, LlmRequest, Message, Role};

// Requires: cargo add wesichain-llm --features openai
let client = OpenAiClient::new(std::env::var("OPENAI_API_KEY").unwrap());

let request = LlmRequest {
    model: "gpt-4o-mini".to_string(),
    messages: vec![Message {
        role: Role::User,
        content: "Hello, world!".into(),
        tool_call_id: None,
        tool_calls: vec![],
    }],
    tools: vec![],
};

let response = client.invoke(request).await?;
println!("{}", response.content);
```

## Key Patterns

### Pattern 1: OpenAI Client with API Key

```rust
use wesichain_llm::OpenAiClient;

// Requires: --features openai
let client = OpenAiClient::new("sk-...");

// Or with a specific model
let client = OpenAiClient::new(api_key)
    .with_model("gpt-4o");
```

### Pattern 2: OpenAI-Compatible Client for Custom Endpoints

```rust
use wesichain_llm::OpenAiCompatibleClient;

// Use for Ollama, self-hosted models, or any OpenAI-compatible API
let client = OpenAiCompatibleClient::builder()
    .base_url("http://localhost:11434")?  // Ollama
    .api_key("ollama")  // Ollama accepts any key
    .default_model("llama3.1")
    .build()?;
```

### Pattern 3: Ollama Client (Native API)

```rust
use wesichain_llm::OllamaClient;

// Native Ollama API (not OpenAI-compatible)
let client = OllamaClient::new(
    "http://localhost:11434".to_string(),
    "llama3.1".to_string()
)?;
```

### Pattern 4: Tool Calling with ToolSpec

```rust
use wesichain_core::{ToolSpec, ToolParameter, ToolParameterType};
use wesichain_llm::{LlmRequest, Message, Role};
use serde_json::json;

let weather_tool = ToolSpec {
    name: "get_weather".to_string(),
    description: "Get weather for a location".to_string(),
    parameters: vec![
        ToolParameter {
            name: "location".to_string(),
            description: "City name".to_string(),
            required: true,
            parameter_type: ToolParameterType::String,
        },
    ],
};

let request = LlmRequest {
    model: "gpt-4o-mini".to_string(),
    messages: vec![Message {
        role: Role::User,
        content: "What's the weather in Paris?".into(),
        tool_call_id: None,
        tool_calls: vec![],
    }],
    tools: vec![weather_tool],
};

let response = client.invoke(request).await?;

// Check for tool calls
for tool_call in &response.tool_calls {
    println!("Tool: {}", tool_call.name);
    println!("Args: {}", tool_call.arguments);
}
```

### Pattern 5: Streaming Responses

```rust
use wesichain_core::Runnable;
use futures::StreamExt;

let mut stream = client.stream(request);

while let Some(event) = stream.next().await {
    match event? {
        StreamEvent::ContentChunk(chunk) => {
            print!("{}", chunk);  // Print as it arrives
        }
        StreamEvent::FinalAnswer(_) => {
            println!("\n[Done]");
        }
        StreamEvent::ToolCallStart { name, .. } => {
            println!("\n[Calling tool: {}]", name);
        }
        _ => {}
    }
}
```

### Pattern 6: Provider-Specific Clients

```rust
// DeepSeek - Requires: --features deepseek
use wesichain_llm::DeepSeekClient;
let client = DeepSeekClient::new("sk-...");

// Google (Gemini) - Requires: --features google
use wesichain_llm::GoogleClient;
let client = GoogleClient::new("api-key");

// Azure OpenAI - Requires: --features azure
use wesichain_llm::AzureOpenAiClient;
let client = AzureOpenAiClient::new("endpoint", "api-key", "deployment");

// Mistral - Requires: --features mistral
use wesichain_llm::MistralClient;
let client = MistralClient::new("api-key");
```

### Pattern 7: Using with Runnable Chains

```rust
use wesichain_core::{Runnable, RunnableExt};

// LLM clients implement Runnable<LlmRequest, LlmResponse>
let chain = PromptTemplate::new("Tell me about {topic}")
    .bind(json!({"topic": "Rust"}))
    .then(client)
    .then(StrOutputParser);

let answer: String = chain.invoke(()).await?;
```

## Golden Rules

1. **Use provider feature flags** - Enable only what you need: `openai`, `deepseek`, `google`, `azure`, `mistral`
2. **Always check for tool calls** - After invoke(), check `response.tool_calls` before using `response.content`
3. **Handle API keys securely** - Use environment variables or secrets management, never hardcode
4. **Use OpenAiCompatibleClient for generic providers** - Self-hosted models, custom endpoints, or any OpenAI-compatible API
5. **Set appropriate timeouts** - DeepSeek may need 300s, OpenAI typically 60s
6. **Implement ToolCallingLlm** - If building a custom LLM wrapper, implement this trait for tool support
7. **Stream for better UX** - Use `.stream()` instead of `.invoke()` for real-time response display

## Common Mistakes

- **Forgetting feature flags** - Must enable features like `--features openai` for provider clients
- **Wrong base_url format** - Use full URL including protocol: `https://api.openai.com` not `api.openai.com`
- **Ignoring tool_calls in response** - Tool calls come in `response.tool_calls`, not in `response.content`
- **Not setting default model** - Some requests fail without a model specified in LlmRequest or client default
- **Blocking in stream handlers** - Stream processing must be async; use `.await` properly
- **Mixing Ollama APIs** - `OllamaClient` uses native Ollama API; for OpenAI-compatible Ollama, use `OpenAiCompatibleClient`

## Resources

- Crate: `wesichain-llm`
- Feature flags: `openai`, `deepseek`, `google`, `azure`, `mistral`
- Key types: `LlmRequest`, `LlmResponse`, `ToolSpec`, `ToolCall`, `Message`, `Role`
- Key clients: `OpenAiClient`, `OpenAiCompatibleClient`, `OllamaClient`, `DeepSeekClient`, `GoogleClient`, `AzureOpenAiClient`, `MistralClient`
- Trait: `ToolCallingLlm` (for advanced tool calling features)
