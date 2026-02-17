//! # wesichain-agent
//!
//! > **⚠️ Important:** The `AgentExecutor` and `ToolCallingAgent` in this crate are deprecated  
//! > as of **v0.3.0** and will be **removed in v0.4.0**.  
//! > For production use, migrate to [`ReActAgentNode`](https://docs.rs/wesichain-graph/latest/wesichain_graph/struct.ReActAgentNode.html) in `wesichain-graph`.
//!
//! ## Migration Guide
//!
//! ### Before (Deprecated)
//!
//! ```rust,ignore
//! use wesichain_agent::{AgentExecutor, ToolCallingAgent};
//! use wesichain_llm::PromptTemplate;
//! use std::sync::Arc;
//!
//! let prompt = PromptTemplate::new("Answer the question: {input}".to_string());
//! let agent = ToolCallingAgent::new(
//!     Arc::new(my_llm),
//!     vec![Arc::new(calculator), Arc::new(search)],
//!     prompt,
//! );
//! let executor = AgentExecutor::new(agent, vec![Arc::new(calculator), Arc::new(search)])
//!     .with_max_iterations(10);
//!
//! let result = executor.invoke(llm_request).await?;
//! ```
//!
//! ### After (Recommended)
//!
//! ```rust,ignore
//! use wesichain_graph::ReActAgentNode;
//! use std::sync::Arc;
//!
//! let agent = ReActAgentNode::builder()
//!     .llm(Arc::new(my_llm))
//!     .tools(vec![Arc::new(calculator), Arc::new(search)])
//!     .max_iterations(10)
//!     .build()?;
//!
//! // Use within a graph or standalone
//! let result = agent.invoke(input_state).await?;
//! ```
//!
//! ## Why Migrate?
//!
//! `ReActAgentNode` provides:
//! - ✅ **True Thought/Action/Observation ReAct loop** (not just tool calling)
//! - ✅ **Scratchpad state** with `ReActStep` enum for introspection
//! - ✅ **Tool failure policies**: `FailFast` or `AppendErrorAndContinue`
//! - ✅ **Observer integration** for tracing and debugging
//! - ✅ **Proper tool call handling** from LLM responses
//!
//! See [`wesichain-graph` documentation](https://docs.rs/wesichain-graph) for details.

mod action;
mod agent;
mod executor;
mod factory;
mod tool;

pub use action::{ActionAgent, AgentAction, AgentFinish, AgentStep};
#[allow(deprecated)]
pub use agent::ToolCallingAgent;
#[allow(deprecated)]
pub use executor::AgentExecutor;
pub use factory::create_tool_calling_agent;
pub use tool::ToolRegistry;
pub use wesichain_core::Tool;
