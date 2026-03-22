# wesichain-mcp

Model Context Protocol (MCP 2024-11-05) client for Wesichain — stdio and HTTP/SSE transports.

Part of the [wesichain](https://github.com/wesichain/wesichain) LLM agent framework.

## Installation

```toml
[dependencies]
wesichain-mcp = "0.3"
```

## Quick Start

```rust
use wesichain_mcp::{McpClient, StdioTransport};
use wesichain_agent::tooling::ToolRegistry;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to an MCP server over stdio
    let transport = StdioTransport::spawn("my-mcp-server", &[]).await?;
    let client = McpClient::new(transport).await?;

    // List and register tools from the MCP server
    let tools = client.list_tools().await?;
    println!("Available MCP tools: {}", tools.len());

    Ok(())
}
```

## Transports

| Transport | Use case |
|-----------|----------|
| `StdioTransport` | Local processes (subprocess MCP servers) |
| `HttpTransport` | Remote MCP servers over HTTP + SSE |

## MCP Resources & Sampling

The client also supports MCP `resources/list`, `resources/read`, and `sampling/createMessage` for agentic use cases.

## License

Apache-2.0 OR MIT
