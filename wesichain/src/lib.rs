//! Wesichain — Rust-native LLM agents & chains.
//!
//! # Feature flags
//!
//! | Feature | What it enables |
//! |---------|----------------|
//! | `openai` | OpenAI-compatible LLM client (`wesichain-llm`) |
//! | `anthropic` | Anthropic Claude client with extended thinking |
//! | `agent` | ReAct agent runtime with FSM, tool dispatch, checkpointing |
//! | `graph` | Stateful execution graphs with persistence |
//! | `tools` | Built-in coding tools: file I/O, bash, glob, grep, git, patch |
//! | `session` | Session persistence, cost tracking, token budget |
//! | `mcp` | Model Context Protocol client (stdio + HTTP) |
//! | `memory` | Short-term + semantic (vector-backed) conversation memory |
//! | `retrieval` | RAG: document loaders, splitters, retrievers, re-ranking |
//! | `server` | Axum HTTP server with auth middleware |
//! | `langsmith` | LangSmith observability integration |
//! | `langfuse` | Langfuse observability integration |
//! | `coding` | Alias: `tools + agent + session + mcp` |
//! | `full` | All of the above |

pub use wesichain_core as core;
pub use wesichain_prompt as prompt;

#[cfg(feature = "agent")]
pub use wesichain_agent as agent;
#[cfg(feature = "graph")]
pub use wesichain_graph as graph;
#[cfg(feature = "openai")]
pub use wesichain_llm as llm;
#[cfg(feature = "anthropic")]
pub use wesichain_anthropic as anthropic;
#[cfg(feature = "langsmith")]
pub use wesichain_langsmith as langsmith;
#[cfg(feature = "langfuse")]
pub use wesichain_langfuse as langfuse;
#[cfg(feature = "qdrant")]
pub use wesichain_qdrant as qdrant;
#[cfg(feature = "tools")]
pub use wesichain_tools as tools;
#[cfg(feature = "server")]
pub use wesichain_server as server;
#[cfg(feature = "mcp")]
pub use wesichain_mcp as mcp;
#[cfg(feature = "session")]
pub use wesichain_session as session;
#[cfg(feature = "memory")]
pub use wesichain_memory as memory;
#[cfg(feature = "retrieval")]
pub use wesichain_retrieval as retrieval;

pub mod prelude {
    // ── Core ──────────────────────────────────────────────────────────────────
    pub use wesichain_core::prelude::*;

    // Model capabilities & token budget (Sprint 3 / Sprint 6)
    pub use wesichain_core::capability::{for_model as capabilities_for, ModelCapabilities};
    pub use wesichain_core::token_budget::TokenBudget;

    // ── Prompt ────────────────────────────────────────────────────────────────
    pub use wesichain_prompt::*;

    // ── Agent ─────────────────────────────────────────────────────────────────
    #[cfg(feature = "agent")]
    pub use wesichain_agent::{
        AgentCheckpoint, AgentRuntime, CancellationToken, ToolContext, ToolError, ToolSet,
        TypedTool,
    };

    // ── LLM providers ────────────────────────────────────────────────────────
    #[cfg(feature = "openai")]
    pub use wesichain_llm::OpenAiCompatibleClient;

    #[cfg(feature = "anthropic")]
    pub use wesichain_anthropic::AnthropicClient;

    // ── Graph ────────────────────────────────────────────────────────────────
    #[cfg(feature = "graph")]
    pub use wesichain_graph::{GraphBuilder, StateSchema};

    // ── Tools ────────────────────────────────────────────────────────────────
    #[cfg(feature = "tools")]
    pub use wesichain_tools::{
        EditFileTool, GlobTool, GrepTool, PatchTool, PathGuard, ReadFileTool,
        ReplaceLinesTool, ToolBundle, WorkspaceDetectorTool, WriteFileTool,
    };

    #[cfg(all(feature = "tools", feature = "exec"))]
    pub use wesichain_tools::BashExecTool;

    #[cfg(all(feature = "tools", feature = "git"))]
    pub use wesichain_tools::{GitDiffTool, GitStatusTool, GitLogTool, GitCommitTool};

    // ── Session ───────────────────────────────────────────────────────────────
    #[cfg(feature = "session")]
    pub use wesichain_session::{
        cost_for_response, price_for_model, FileSessionStore, Session, SessionCostSummary,
        SessionManager,
    };

    // ── MCP ───────────────────────────────────────────────────────────────────
    #[cfg(feature = "mcp")]
    pub use wesichain_mcp::{McpClient, McpTool, StdioTransport, ToolSetBuilderMcpExt};

    // ── Memory ────────────────────────────────────────────────────────────────
    #[cfg(feature = "memory")]
    pub use wesichain_memory::{EntityMemory, Memory, MemoryRouter, VectorMemoryStore};

    // ── Retrieval ─────────────────────────────────────────────────────────────
    #[cfg(feature = "retrieval")]
    pub use wesichain_retrieval::{
        CrossEncoderRetriever, InMemoryVectorStore, KeywordReranker, Reranker, Retriever,
    };

    // ── Observability ─────────────────────────────────────────────────────────
    #[cfg(feature = "langsmith")]
    pub use wesichain_langsmith::{LangSmithCallbackHandler, LangSmithConfig};

    #[cfg(feature = "langfuse")]
    pub use wesichain_langfuse::{LangfuseCallbackHandler, LangfuseConfig};
}
