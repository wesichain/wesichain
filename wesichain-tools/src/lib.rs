//! Built-in tool library for wesichain.
//!
//! Provides ready-to-use tools that every project needs:
//! - **http**: `HttpGetTool`, `HttpPostTool` (feature `http`, on by default)
//! - **fs**: `ReadFileTool`, `WriteFileTool`, `EditFileTool`, `GlobTool`, `GrepTool`,
//!   `PatchTool` (feature `fs`, on by default)
//! - **search**: `TavilySearchTool` via Tavily API (feature `search`)
//! - **exec**: `BashExecTool` (feature `exec`, off by default)
//!
//! # Quick start
//! ```ignore
//! use wesichain_tools::ToolBundle;
//! use wesichain_agent::ToolSet;
//!
//! let tools = ToolBundle::all_default().build()?;
//! ```

#[cfg(feature = "http")]
pub mod http_client;

#[cfg(feature = "fs")]
pub mod edit;
#[cfg(feature = "fs")]
pub mod file_system;
#[cfg(feature = "fs")]
pub mod glob_tool;
#[cfg(feature = "fs")]
pub mod grep_tool;
#[cfg(feature = "fs")]
pub mod patch;
#[cfg(feature = "fs")]
pub mod workspace;

#[cfg(feature = "search")]
pub mod search;

#[cfg(feature = "exec")]
pub mod exec;

#[cfg(feature = "git")]
pub mod git;

// Flat re-exports
#[cfg(feature = "http")]
pub use http_client::{HttpGetTool, HttpPostTool};

#[cfg(feature = "fs")]
pub use edit::{EditFileTool, ReplaceLinesTool};
#[cfg(feature = "fs")]
pub use file_system::{PathGuard, ReadFileTool, WriteFileTool};
#[cfg(feature = "fs")]
pub use glob_tool::GlobTool;
#[cfg(feature = "fs")]
pub use grep_tool::GrepTool;
#[cfg(feature = "fs")]
pub use patch::PatchTool;
#[cfg(feature = "fs")]
pub use workspace::{WorkspaceDetectorTool, WorkspaceInfo, ProjectKind};

#[cfg(feature = "search")]
pub use search::TavilySearchTool;

#[cfg(feature = "exec")]
pub use exec::BashExecTool;

#[cfg(feature = "git")]
pub use git::{GitBlameTool, GitCommitTool, GitDiffTool, GitLogTool, GitStatusTool};

// Re-export ToolSetBuilder so callers don't need wesichain-agent directly
pub use wesichain_agent::tooling::ToolSetBuilder;

/// Convenience bundle for registering the default set of tools.
pub struct ToolBundle;

impl ToolBundle {
    /// Returns a [`ToolSetBuilder`] pre-loaded with HTTP + filesystem tools.
    ///
    /// Add `search` or `exec` features to include those tools, then call
    /// `.build()` on the returned builder.
    pub fn all_default() -> ToolSetBuilder {
        #[allow(unused_mut)]
        let mut builder = wesichain_agent::ToolSet::new();

        #[cfg(feature = "http")]
        {
            builder = builder
                .register_with(HttpGetTool)
                .register_with(HttpPostTool);
        }

        #[cfg(feature = "fs")]
        {
            builder = builder
                .register_with(ReadFileTool)
                .register_with(WriteFileTool)
                .register_with(EditFileTool)
                .register_with(GlobTool)
                .register_with(GrepTool)
                .register_with(PatchTool);
        }

        #[cfg(feature = "search")]
        {
            builder = builder.register_with(TavilySearchTool::from_env());
        }

        builder
    }

    /// Returns a [`ToolSetBuilder`] with the full coding-agent toolkit:
    /// `EditFileTool`, `ReplaceLinesTool`, `GlobTool`, `GrepTool`, `PatchTool`,
    /// `ReadFileTool`, `WriteFileTool`, `WorkspaceDetectorTool`.
    #[cfg(feature = "fs")]
    pub fn coding_tools() -> ToolSetBuilder {
        wesichain_agent::ToolSet::new()
            .register_with(ReadFileTool)
            .register_with(WriteFileTool)
            .register_with(EditFileTool)
            .register_with(ReplaceLinesTool)
            .register_with(GlobTool)
            .register_with(GrepTool)
            .register_with(PatchTool)
            .register_with(WorkspaceDetectorTool)
    }
}
