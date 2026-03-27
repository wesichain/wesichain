---
name: wesichain-memory
description: |
  Memory management for conversation state in Wesichain. ConversationBufferMemory,
  ConversationSummaryMemory, ConversationWindowMemory, VectorMemoryStore, EntityMemory,
  and MemoryRouter for routing between memory layers. Use when managing chat history
  and state across conversation turns.
triggers:
  - "memory"
  - "conversation"
  - "buffer"
  - "summary"
  - "window"
  - "wesichain-memory"
  - "chat history"
  - "thread_id"
  - "EntityMemory"
  - "MemoryRouter"
  - "VectorMemoryStore"
---

## When to Use

Use wesichain-memory when you need to:
- Persist conversation history across multiple turns with a user
- Manage token budgets by summarizing or windowing older messages
- Implement semantic memory that retrieves relevant past conversations
- Store extracted entities (user name, preferences, facts) for later use
- Combine multiple memory strategies (short-term + long-term)

Choose the right memory type:
- **ConversationBufferMemory**: Short conversations, need full context
- **ConversationSummaryMemory**: Long conversations, bounded token usage
- **ConversationWindowMemory**: Fixed context window, predictable latency
- **VectorMemoryStore**: Semantic similarity, long-term knowledge
- **EntityMemory**: Key facts about user/preferences
- **MemoryRouter**: Combine multiple memory types together

## Quick Start

```rust
use std::sync::Arc;
use wesichain_core::checkpoint::FileCheckpointer;
use wesichain_memory::{ConversationBufferMemory, Memory};

// Create a checkpointer for persistence
let checkpointer = Arc::new(FileCheckpointer::new("./checkpoints"));

// Create buffer memory
let memory = ConversationBufferMemory::new(checkpointer);

// Save a conversation turn
let mut inputs = HashMap::new();
inputs.insert("input", "What's the weather?".into());
let mut outputs = HashMap::new();
outputs.insert("output", "It's sunny today!".into());

memory.save_context("thread-123", &inputs, &outputs).await?;

// Load memory for the next turn
let vars = memory.load_memory_variables("thread-123").await?;
let history = vars.get("history");
```

## Key Patterns

### Pattern 1: Full Conversation History with ConversationBufferMemory

Use when you need complete conversation context and token count is manageable.

```rust
use std::sync::Arc;
use wesichain_core::checkpoint::FileCheckpointer;
use wesichain_memory::{ConversationBufferMemory, Memory};
use wesichain_llm::Message;

let checkpointer = Arc::new(FileCheckpointer::new("./checkpoints"));
let memory = ConversationBufferMemory::new(checkpointer)
    .with_prefixes("User", "Assistant")     // Customize role prefixes
    .with_memory_key("chat_history");        // Key in variables map

// In your agent loop
let vars = memory.load_memory_variables(&thread_id).await?;
let messages: Vec<Message> = serde_json::from_value(
    vars.get("chat_history").unwrap().clone()
)?;

// Messages are ready to pass to LLM
```

### Pattern 2: Summarize Old Messages with ConversationSummaryMemory

Use for long conversations where full history would exceed token limits.

```rust
use std::sync::Arc;
use wesichain_core::checkpoint::FileCheckpointer;
use wesichain_core::{LlmRequest, LlmResponse, Runnable};
use wesichain_memory::{ConversationSummaryMemory, Memory};

let checkpointer = Arc::new(FileCheckpointer::new("./checkpoints"));
let llm: Arc<dyn Runnable<LlmRequest, LlmResponse>> = // your LLM

let memory = ConversationSummaryMemory::new(checkpointer, llm)
    .with_buffer_size(4)                        // Keep last 4 messages unsummarized
    .with_memory_key("history")
    .with_summarization_prompt("Summarize: {summary}\n\nNew lines: {new_lines}\n\nSummary:");

// Returns: summary + recent buffer messages
let vars = memory.load_memory_variables(&thread_id).await?;
let history = vars.get("history").unwrap().as_str().unwrap();
// Format: "Summary of earlier conversation:\n...\n\nRecent conversation:\n..."
```

### Pattern 3: Fixed Window with ConversationWindowMemory

Use when you need predictable, bounded context size.

```rust
use std::sync::Arc;
use wesichain_core::checkpoint::FileCheckpointer;
use wesichain_memory::{ConversationWindowMemory, Memory};

let checkpointer = Arc::new(FileCheckpointer::new("./checkpoints"));

// Keep only the last 10 messages (5 turns)
let memory = ConversationWindowMemory::new(checkpointer, 10)
    .with_prefixes("Human", "AI")
    .with_memory_key("history");

// Older messages are automatically discarded
memory.save_context(&thread_id, &inputs, &outputs).await?;
```

### Pattern 4: Semantic Memory with VectorMemoryStore

Use for long-term memory that retrieves relevant conversations by meaning, not just recency.

```rust
use std::sync::Arc;
use wesichain_core::{Embedding, VectorStore};
use wesichain_memory::{VectorMemoryStore, Memory};

let embedder: Arc<dyn Embedding> = // e.g., OpenAI embeddings
let vector_store: Arc<dyn VectorStore> = // e.g., Qdrant, Chroma

let memory = VectorMemoryStore::new(embedder, vector_store, 3)
    .with_memory_key("relevant_history");

// Save turns as they happen
memory.save_context(&thread_id, &inputs, &outputs).await?;

// Retrieve top 3 most semantically similar past turns
// Pass the user's query as thread_id for contextual retrieval
let vars = memory.load_memory_variables(&user_query).await?;
let relevant: Vec<String> = vars.get("relevant_history")
    .unwrap()
    .as_array()
    .unwrap()
    .iter()
    .map(|v| v.as_str().unwrap().to_string())
    .collect();
```

### Pattern 5: Entity Extraction with EntityMemory

Use for storing structured facts about the user or context.

```rust
use wesichain_memory::{EntityMemory, Memory};
use serde_json::json;

let memory = EntityMemory::new();

// Direct upsert for known entities
memory.upsert(&thread_id, "user_name", json!("Alice"));
memory.upsert(&thread_id, "project", json!("wesichain"));

// Or save from LLM outputs
let mut outputs = HashMap::new();
outputs.insert("user_name", json!("Alice"));
outputs.insert("preference", json!("rust"));
memory.save_context(&thread_id, &HashMap::new(), &outputs).await?;

// Load all entities
let vars = memory.load_memory_variables(&thread_id).await?;
let entities = vars.get("entities").unwrap();
// {"user_name": "Alice", "preference": "rust", "project": "wesichain"}
```

### Pattern 6: Combine Memory Layers with MemoryRouter

Use when you need multiple memory strategies working together.

```rust
use std::sync::Arc;
use wesichain_memory::{MemoryRouter, Memory};

let buffer_memory: Arc<dyn Memory> = Arc::new(buffer_mem);
let entity_memory: Arc<dyn Memory> = Arc::new(entity_mem);
let vector_memory: Arc<dyn Memory> = Arc::new(vector_mem);

// Fan out to all layers - each handles what it's best at
let router = MemoryRouter::new(vec![])
    .push(buffer_memory)    // Recent conversation
    .push(entity_memory)    // User facts
    .push(vector_memory);   // Relevant past conversations

// save_context goes to ALL layers
router.save_context(&thread_id, &inputs, &outputs).await?;

// load_memory_variables merges all layers
let vars = router.load_memory_variables(&thread_id).await?;
// Contains: history (from buffer), entities (from entity), relevant_history (from vector)
```

### Pattern 7: Implement Custom Memory

Use when you need custom storage or retrieval logic.

```rust
use async_trait::async_trait;
use wesichain_memory::Memory;
use wesichain_core::WesichainError;

pub struct RedisMemory {
    client: redis::Client,
    ttl: u64,
}

#[async_trait]
impl Memory for RedisMemory {
    async fn load_memory_variables(
        &self,
        thread_id: &str,
    ) -> Result<HashMap<String, Value>, WesichainError> {
        // Custom retrieval logic
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| WesichainError::Custom(e.to_string()))?;
        
        let data: String = redis::cmd("GET")
            .arg(format!("memory:{}", thread_id))
            .query_async(&mut conn)
            .await
            .map_err(|e| WesichainError::Custom(e.to_string()))?;
        
        serde_json::from_str(&data)
            .map_err(|e| WesichainError::Custom(e.to_string()))
    }

    async fn save_context(
        &self,
        thread_id: &str,
        inputs: &HashMap<String, Value>,
        outputs: &HashMap<String, Value>,
    ) -> Result<(), WesichainError> {
        // Custom save logic with TTL
        // Implementation...
        Ok(())
    }

    async fn clear(&self, thread_id: &str) -> Result<(), WesichainError> {
        // Clear logic...
        Ok(())
    }
}
```

## Golden Rules

1. **Always use a Checkpointer** - Memory requires persistence via FileCheckpointer or custom implementation
2. **Choose memory based on conversation length** - Buffer for short, Summary/Window for long, Vector for semantic
3. **Use EntityMemory for facts** - Store structured data (names, preferences) separately from chat history
4. **Combine with MemoryRouter** - Use multiple memory types together for best results
5. **Thread isolation** - Always use unique thread_id per conversation to prevent data leakage
6. **Handle the memory_key** - Know what key your memory type returns (default: "history")

## Common Mistakes

- **Forgetting to save context** - Call `save_context` after every LLM response
- **Using wrong thread_id** - Each conversation needs its own unique thread_id
- **Expecting Message objects from all memories** - Summary returns a String, not Vec<Message>
- **Not handling empty memory** - First turn has no history; handle gracefully
- **Assuming VectorMemoryStore clears** - clear() is a no-op; manage vector store directly
- **Passing wrong keys to save_context** - Use "input"/"output" or customize extraction logic
- **Not Arc-wrapping checkpointer** - Memory types require Arc<C> for thread safety

## Resources

- Crate: `wesichain-memory`
- Trait: `Memory`
- Buffer memory: `ConversationBufferMemory`
- Summary memory: `ConversationSummaryMemory`
- Window memory: `ConversationWindowMemory`
- Semantic memory: `VectorMemoryStore`
- Entity storage: `EntityMemory`
- Multi-layer: `MemoryRouter`
- Checkpointer: `wesichain_core::checkpoint::{Checkpointer, FileCheckpointer}`
