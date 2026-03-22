//! Stdio transport — spawns a subprocess and communicates over stdin/stdout
//! using newline-delimited JSON-RPC 2.0 messages.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

use crate::error::McpError;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::transport::McpTransport;

pub struct StdioTransport {
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    next_id: AtomicI64,
    _child: Arc<Mutex<Child>>,
}

impl StdioTransport {
    /// Spawn `command args` and create a transport over its stdio.
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Self, McpError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take().expect("stdin not captured");
        let stdout = child.stdout.take().expect("stdout not captured");

        let transport = Self {
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: Arc::new(Mutex::new(BufReader::new(stdout))),
            next_id: AtomicI64::new(1),
            _child: Arc::new(Mutex::new(child)),
        };

        // Send MCP initialize handshake
        transport.initialize().await?;

        Ok(transport)
    }

    async fn initialize(&self) -> Result<(), McpError> {
        let req = JsonRpcRequest::new(
            self.next_id.fetch_add(1, Ordering::SeqCst),
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "wesichain-mcp", "version": "0.2.1" }
            })),
        );
        let resp = self.send_raw(req).await?;
        if resp.error.is_some() {
            return Err(McpError::Protocol("MCP initialize failed".to_string()));
        }
        // Send initialized notification
        let notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let line = serde_json::to_string(&notif)? + "\n";
        self.stdin.lock().await.write_all(line.as_bytes()).await?;
        Ok(())
    }

    async fn send_raw(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, McpError> {
        let line = serde_json::to_string(&request)? + "\n";
        self.stdin.lock().await.write_all(line.as_bytes()).await?;

        let mut response_line = String::new();
        self.stdout.lock().await.read_line(&mut response_line).await?;

        if response_line.is_empty() {
            return Err(McpError::Closed);
        }

        let resp: JsonRpcResponse = serde_json::from_str(response_line.trim())?;
        Ok(resp)
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn send(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, McpError> {
        self.send_raw(request).await
    }
}
