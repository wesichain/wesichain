//! MCP (Model Context Protocol) client for wesichain.
//!
//! Connects to any MCP server via stdio transport and loads its tools into a
//! [`wesichain_agent::ToolSet`].
//!
//! # Quick start
//! ```ignore
//! use wesichain_agent::ToolSet;
//! use wesichain_mcp::bridge::ToolSetBuilderMcpExt;
//!
//! let tools = ToolSet::new()
//!     .add_mcp_server("npx", &["-y", "@modelcontextprotocol/server-filesystem", "."])
//!     .await?
//!     .build()?;
//! ```

pub mod bridge;
pub mod error;
pub mod http;
pub mod protocol;
pub mod stdio;
pub mod transport;

pub use bridge::{load_mcp_tools, McpClient, McpTool, ToolSetBuilderMcpExt};
pub use error::McpError;
pub use http::HttpMcpTransport;
pub use protocol::{McpResourceContent, McpResourceSpec, McpToolSpec, SamplingMessage, SamplingRequest, SamplingResult};
pub use stdio::StdioTransport;
pub use transport::McpTransport;
