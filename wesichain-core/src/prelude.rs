// Prelude module for wesichain-core
//
// Import commonly used types with: `use wesichain_core::prelude::*;`

pub use crate::{
    // Output parsers
    BaseOutputParser,
    CallbackManager,
    Chain,
    // Documents
    Document,
    // Embeddings & Vector Stores
    Embedding,
    EmbeddingError,
    HasFinalOutput,

    HasUserInput,
    IntoValue,
    JsonOutputParser,
    // LLM primitives
    LlmRequest,
    LlmResponse,
    Message,
    // Metadata
    MetadataFilter,

    OutputFixingParser,

    // State & ReAct
    ReActStep,
    Retrying,

    Role,
    // Callbacks
    RunConfig,
    RunContext,
    RunType,
    // Core traits
    Runnable,
    RunnableExt,
    // Fallbacks & retries
    RunnableWithFallbacks,
    RuntimeChain,

    ScratchpadState,
    SearchResult,

    StoreError,

    StrOutputParser,
    StreamEvent,
    StructuredOutputParser,
    // Tools
    Tool,
    ToolCall,
    ToolCallingLlm,
    ToolCallingLlmExt,

    ToolError,

    ToolSpec,
    TryFromValue,

    Value,
    VectorStore,
    // Errors
    WesichainError,
};
