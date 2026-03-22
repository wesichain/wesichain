//! `BashExecTool` — streaming shell execution (feature `exec`).
//!
//! Replaces the old buffered `cmd.output()` approach with `spawn()` +
//! `tokio::io::BufReader` so that long-running commands (e.g. `cargo build`)
//! stream their output line-by-line as `StreamEvent::ContentChunk` events
//! rather than appearing to hang until completion.
//!
//! Streaming only happens when the caller has attached a `stream_tx` to the
//! [`ToolContext`].  The full accumulated stdout/stderr are still returned in
//! [`BashExecOutput`] regardless.

use std::process::Stdio;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use wesichain_core::{StreamEvent, ToolContext, ToolError, TypedTool};

/// Default timeout for shell commands (5 minutes — covers `cargo build`).
const DEFAULT_TIMEOUT_SECS: u64 = 300;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BashExecArgs {
    /// Shell command to run (passed to `bash -c`).
    pub command: String,
    /// Working directory (defaults to CWD).
    pub working_dir: Option<String>,
    /// Maximum seconds to wait before killing the process (default: 300).
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BashExecOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Clone, Default)]
pub struct BashExecTool;

#[async_trait::async_trait]
impl TypedTool for BashExecTool {
    type Args = BashExecArgs;
    type Output = BashExecOutput;
    const NAME: &'static str = "bash_exec";

    async fn run(&self, args: Self::Args, ctx: ToolContext) -> Result<Self::Output, ToolError> {
        let timeout_dur = std::time::Duration::from_secs(
            args.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS),
        );

        let mut cmd = tokio::process::Command::new("bash");
        cmd.arg("-c").arg(&args.command);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        if let Some(dir) = &args.working_dir {
            cmd.current_dir(dir);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| ToolError::ExecutionFailed(format!("failed to spawn process: {e}")))?;

        let stdout_pipe = child.stdout.take().expect("stdout was piped");
        let stderr_pipe = child.stderr.take().expect("stderr was piped");

        // Stream stdout line-by-line, sending chunks to the event channel if connected.
        let stream_tx = ctx.stream_tx.clone();
        let stdout_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout_pipe).lines();
            let mut buf = String::new();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(ref tx) = stream_tx {
                    let _ = tx.send(StreamEvent::ContentChunk(format!("{line}\n")));
                }
                buf.push_str(&line);
                buf.push('\n');
            }
            buf
        });

        // Drain stderr silently (no per-line streaming for stderr).
        let stderr_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr_pipe).lines();
            let mut buf = String::new();
            while let Ok(Some(line)) = reader.next_line().await {
                buf.push_str(&line);
                buf.push('\n');
            }
            buf
        });

        // Wait for the process to exit, respecting timeout and cancellation.
        let status = tokio::select! {
            result = tokio::time::timeout(timeout_dur, child.wait()) => {
                result
                    .map_err(|_| {
                        ToolError::ExecutionFailed(format!(
                            "command timed out after {}s",
                            timeout_dur.as_secs()
                        ))
                    })?
                    .map_err(|e| ToolError::ExecutionFailed(format!("wait failed: {e}")))?
            }
            _ = ctx.cancellation.cancelled() => {
                child.kill().await.ok();
                return Err(ToolError::ExecutionFailed("command was cancelled".to_string()));
            }
        };

        let stdout = stdout_task.await.unwrap_or_default();
        let stderr = stderr_task.await.unwrap_or_default();

        Ok(BashExecOutput {
            stdout,
            stderr,
            exit_code: status.code().unwrap_or(-1),
        })
    }
}
