// Prelude module for wesichain-core
//
// Import commonly used types with: `use wesichain_core::prelude::*;`

pub use crate::{
    // Model capabilities & token budget
    capability::ModelCapabilities,
    token_budget::TokenBudget,

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
    CancellationToken,
    Tool,
    ToolCall,
    ToolCallingLlm,
    ToolCallingLlmExt,
    ToolContext,
    ToolError,
    TypedTool,

    ToolSpec,
    TryFromValue,

    Value,
    VectorStore,
    // Errors
    WesichainError,
};
