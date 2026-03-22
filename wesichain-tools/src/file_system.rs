//! Filesystem tools: read and write files within the allowed directory.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wesichain_core::{ToolContext, ToolError, TypedTool};

// ── ReadFileTool ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// Path to the file (relative to CWD or absolute)
    pub path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadFileOutput {
    pub path: String,
    pub contents: String,
}

#[derive(Clone, Default)]
pub struct ReadFileTool;

#[async_trait::async_trait]
impl TypedTool for ReadFileTool {
    type Args = ReadFileArgs;
    type Output = ReadFileOutput;
    const NAME: &'static str = "read_file";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        validate_path(&args.path)?;
        let contents = std::fs::read_to_string(&args.path)?;
        Ok(ReadFileOutput { path: args.path, contents })
    }
}

// ── WriteFileTool ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteFileArgs {
    /// Path to write to (relative to CWD or absolute)
    pub path: String,
    /// Content to write
    pub content: String,
    /// When true, return the diff without writing the file (default: false).
    /// For new files the diff shows the entire content as additions.
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct WriteFileOutput {
    /// `true` when the file was written, `false` for a dry run.
    pub ok: bool,
    pub bytes_written: usize,
    /// Unified diff of the change (always populated).
    pub diff: String,
}

#[derive(Clone, Default)]
pub struct WriteFileTool;

#[async_trait::async_trait]
impl TypedTool for WriteFileTool {
    type Args = WriteFileArgs;
    type Output = WriteFileOutput;
    const NAME: &'static str = "write_file";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        validate_path(&args.path)?;

        // Read existing content for diff (empty string for new files).
        let original = std::fs::read_to_string(&args.path).unwrap_or_default();
        let diff = crate::edit::unified_diff(&args.path, &original, &args.content);

        if args.dry_run {
            return Ok(WriteFileOutput { ok: false, bytes_written: 0, diff });
        }

        let bytes = args.content.len();
        std::fs::write(&args.path, &args.content)?;
        Ok(WriteFileOutput { ok: true, bytes_written: bytes, diff })
    }
}

// ── PathGuard ────────────────────────────────────────────────────────────────

/// Sandbox that confines file operations to a root directory.
///
/// Uses `canonicalize()` to resolve symlinks and `..` components, then
/// verifies the resolved path starts within `root`. Works for both existing
/// paths and new paths (writes) by canonicalizing the parent directory.
pub struct PathGuard {
    root: std::path::PathBuf,
}

impl PathGuard {
    pub fn new(root: std::path::PathBuf) -> Self {
        Self { root }
    }

    /// Create a guard rooted at the current working directory.
    pub fn from_cwd() -> Self {
        Self {
            root: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        }
    }

    /// Resolve `path` and verify it is inside `self.root`.
    ///
    /// Returns the canonical path on success. For paths whose last component
    /// does not exist yet (e.g. a file about to be written), the parent
    /// directory is canonicalized and the filename is appended.
    pub fn check(&self, path: &str) -> Result<std::path::PathBuf, ToolError> {
        let candidate = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            self.root.join(path)
        };

        // Try full canonicalize first; fall back to parent + filename for new paths.
        let canonical = candidate.canonicalize().or_else(|_| {
            candidate
                .parent()
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::NotFound, "path has no parent")
                })
                .and_then(|p| p.canonicalize())
                .map(|p| p.join(candidate.file_name().unwrap_or_default()))
        })
        .map_err(|e| {
            ToolError::InvalidInput(format!("cannot resolve path '{path}': {e}"))
        })?;

        let canonical_root = self.root.canonicalize().map_err(|e| {
            ToolError::InvalidInput(format!("cannot resolve workspace root: {e}"))
        })?;

        if !canonical.starts_with(&canonical_root) {
            return Err(ToolError::InvalidInput(format!(
                "path '{path}' resolves outside the allowed workspace directory"
            )));
        }

        Ok(canonical)
    }
}

/// Reject paths that escape the current working directory.
///
/// Uses [`PathGuard::from_cwd`] with full `canonicalize()` validation —
/// stronger than the old `..`-substring check (blocks `/etc/passwd` etc.).
pub fn validate_path(path: &str) -> Result<(), ToolError> {
    PathGuard::from_cwd().check(path)?;
    Ok(())
}
