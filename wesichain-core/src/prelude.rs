// Prelude module for wesichain-core
//
// Import commonly used types with: `use wesichain_core::prelude::*;`

pub use crate::{
    // Core traits
    Runnable, StreamEvent,
    RunnableExt, Chain, RuntimeChain,
    
    // Embeddings & Vector Stores
    Embedding, VectorStore, SearchResult,
    
    // Documents
    Document, Value, IntoValue, TryFromValue,
    
    // Errors
    WesichainError, EmbeddingError, StoreError,
    
    // LLM primitives
    LlmRequest, LlmResponse, Message, Role, ToolCall, ToolSpec,
    ToolCallingLlm, ToolCallingLlmExt,
    
    // Tools
    Tool, ToolError,
    
    // Metadata
    MetadataFilter,
    
    // Output parsers
    BaseOutputParser, JsonOutputParser, StrOutputParser,
    StructuredOutputParser, OutputFixingParser,
    
    // State & ReAct
    ReActStep, ScratchpadState, HasUserInput, HasFinalOutput,
    
    // Fallbacks & retries
    RunnableWithFallbacks, Retrying,
    
    // Callbacks
    RunConfig, CallbackManager, RunContext, RunType,
};
