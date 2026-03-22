//! Tests for wesichain-tools: ReadFileTool, WriteFileTool, BashExecTool, ToolBundle.

use std::io::Write;

use tokio_util::sync::CancellationToken;
use wesichain_core::{ToolContext, TypedTool};
use wesichain_tools::{ToolBundle, file_system::{ReadFileArgs, ReadFileTool, WriteFileArgs, WriteFileTool}};

fn ctx() -> ToolContext {
    ToolContext {
        correlation_id: "test".to_string(),
        step_id: 0,
        cancellation: CancellationToken::new(),
        stream_tx: None,
    }
}

// ── ReadFileTool ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn read_file_reads_existing_file() {
    // Create the temp file inside CWD so PathGuard::from_cwd() accepts it.
    let mut tmp = tempfile::Builder::new().tempfile_in(".").unwrap();
    write!(tmp, "hello wesichain").unwrap();
    let path = tmp.path().to_str().unwrap().to_string();

    let out = ReadFileTool.run(ReadFileArgs { path: path.clone() }, ctx()).await.unwrap();
    assert_eq!(out.contents, "hello wesichain");
    assert_eq!(out.path, path);
}

#[tokio::test]
async fn read_file_rejects_path_traversal() {
    let err = ReadFileTool
        .run(ReadFileArgs { path: "../secret".to_string() }, ctx())
        .await
        .unwrap_err();
    assert!(
        matches!(err, wesichain_core::ToolError::InvalidInput(_)),
        "expected InvalidInput, got: {err:?}"
    );
}

// ── WriteFileTool ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn write_file_creates_file() {
    // Create the temp dir inside CWD so PathGuard::from_cwd() accepts it.
    let dir = tempfile::Builder::new().tempdir_in(".").unwrap();
    let path = dir.path().join("output.txt");
    let path_str = path.to_str().unwrap().to_string();

    let out = WriteFileTool
        .run(WriteFileArgs { path: path_str.clone(), content: "written!".to_string(), dry_run: false }, ctx())
        .await
        .unwrap();

    assert!(out.ok);
    assert_eq!(out.bytes_written, 8);
    let contents = std::fs::read_to_string(&path).unwrap();
    assert_eq!(contents, "written!");
}

#[tokio::test]
async fn write_file_rejects_traversal() {
    let err = WriteFileTool
        .run(WriteFileArgs { path: "../evil.txt".to_string(), content: "x".to_string(), dry_run: false }, ctx())
        .await
        .unwrap_err();
    assert!(matches!(err, wesichain_core::ToolError::InvalidInput(_)));
}

// ── BashExecTool ──────────────────────────────────────────────────────────────

#[cfg(feature = "exec")]
mod exec_tests {
    use super::*;
    use wesichain_tools::exec::{BashExecArgs, BashExecTool};

    #[tokio::test]
    async fn bash_exec_runs_echo() {
        let out = BashExecTool
            .run(BashExecArgs { command: "echo hello".to_string(), working_dir: None }, ctx())
            .await
            .unwrap();
        assert_eq!(out.stdout.trim(), "hello");
        assert_eq!(out.exit_code, 0);
    }

    #[tokio::test]
    async fn bash_exec_times_out() {
        let err = BashExecTool
            .run(BashExecArgs { command: "sleep 60".to_string(), working_dir: None }, ctx())
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("timed out"),
            "expected 'timed out' in error, got: {msg}"
        );
    }
}

// ── ToolBundle ────────────────────────────────────────────────────────────────

#[test]
fn tool_bundle_builds_ok() {
    ToolBundle::all_default().build().expect("ToolBundle::all_default() should build successfully");
}
