//! File editing tools: exact-string replacement and line-range replacement.
//!
//! Both tools support `dry_run: true` — they compute the change and return a
//! unified diff without touching the file on disk.  This lets a coding agent
//! preview every edit before committing it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wesichain_core::{ToolContext, ToolError, TypedTool};

use crate::file_system::validate_path;

// ── EditFileTool ──────────────────────────────────────────────────────────────

/// Exact-string replacement — the primary single-edit primitive.
///
/// Finds `old_string` exactly once in the file and replaces it with
/// `new_string`.  Returns an error if `old_string` is absent or appears more
/// than once (prevents ambiguous edits).  Set `dry_run: true` to preview the
/// change as a unified diff without writing.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EditFileArgs {
    /// Path to the file (relative to CWD or absolute)
    pub path: String,
    /// Exact string to find — must appear exactly once in the file
    pub old_string: String,
    /// Replacement string
    pub new_string: String,
    /// When true, return the diff without writing the file (default: false)
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct EditFileOutput {
    /// `true` when the file was written, `false` for a dry run.
    pub ok: bool,
    pub bytes_written: usize,
    /// Unified diff of the change (always populated).
    pub diff: String,
}

#[derive(Clone, Default)]
pub struct EditFileTool;

#[async_trait::async_trait]
impl TypedTool for EditFileTool {
    type Args = EditFileArgs;
    type Output = EditFileOutput;
    const NAME: &'static str = "edit_file";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        validate_path(&args.path)?;
        let original = std::fs::read_to_string(&args.path)?;

        let count = original.matches(&args.old_string).count();
        match count {
            0 => {
                return Err(ToolError::InvalidInput(format!(
                    "old_string not found in '{}'",
                    args.path
                )))
            }
            1 => {}
            n => {
                return Err(ToolError::InvalidInput(format!(
                    "old_string appears {n} times in '{}'; must appear exactly once",
                    args.path
                )))
            }
        }

        let new_contents = original.replacen(&args.old_string, &args.new_string, 1);
        let diff = unified_diff(&args.path, &original, &new_contents);

        if args.dry_run {
            return Ok(EditFileOutput { ok: false, bytes_written: 0, diff });
        }

        let bytes = new_contents.len();
        std::fs::write(&args.path, &new_contents)?;
        Ok(EditFileOutput { ok: true, bytes_written: bytes, diff })
    }
}

// ── ReplaceLinesTool ──────────────────────────────────────────────────────────

/// Line-range replacement — unambiguous alternative to exact-string matching.
///
/// Replaces lines `start_line` through `end_line` (both 1-indexed, inclusive)
/// with `new_content`.  Useful when the target string appears multiple times
/// or when the edit is best described by position rather than content.
/// Set `dry_run: true` to preview as a unified diff without writing.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplaceLinesArgs {
    /// Path to the file (relative to CWD or absolute)
    pub path: String,
    /// First line to replace (1-indexed, inclusive)
    pub start_line: u32,
    /// Last line to replace (1-indexed, inclusive)
    pub end_line: u32,
    /// Content to insert in place of the replaced lines.
    /// Should include a trailing newline if the file uses LF line endings.
    pub new_content: String,
    /// When true, return the diff without writing the file (default: false)
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReplaceLinesOutput {
    /// `true` when the file was written, `false` for a dry run.
    pub ok: bool,
    pub bytes_written: usize,
    /// Unified diff of the change (always populated).
    pub diff: String,
}

#[derive(Clone, Default)]
pub struct ReplaceLinesTool;

#[async_trait::async_trait]
impl TypedTool for ReplaceLinesTool {
    type Args = ReplaceLinesArgs;
    type Output = ReplaceLinesOutput;
    const NAME: &'static str = "replace_lines";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        validate_path(&args.path)?;

        if args.start_line == 0 {
            return Err(ToolError::InvalidInput(
                "start_line is 1-indexed; 0 is not valid".to_string(),
            ));
        }
        if args.end_line < args.start_line {
            return Err(ToolError::InvalidInput(format!(
                "end_line ({}) must be >= start_line ({})",
                args.end_line, args.start_line
            )));
        }

        let original = std::fs::read_to_string(&args.path)?;
        let mut lines: Vec<&str> = original.lines().collect();
        let total = lines.len();

        let start = (args.start_line - 1) as usize; // convert to 0-indexed
        let end = args.end_line as usize; // exclusive upper bound

        if start >= total {
            return Err(ToolError::InvalidInput(format!(
                "start_line {} exceeds file length ({} lines)",
                args.start_line, total
            )));
        }
        let end = end.min(total);

        // Replace the line range with the new content's lines.
        let new_lines: Vec<&str> = args.new_content.lines().collect();
        lines.splice(start..end, new_lines);

        let new_contents = lines.join("\n") + if original.ends_with('\n') { "\n" } else { "" };
        let diff = unified_diff(&args.path, &original, &new_contents);

        if args.dry_run {
            return Ok(ReplaceLinesOutput { ok: false, bytes_written: 0, diff });
        }

        let bytes = new_contents.len();
        std::fs::write(&args.path, &new_contents)?;
        Ok(ReplaceLinesOutput { ok: true, bytes_written: bytes, diff })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Generate a unified diff between `original` and `modified` using the `diffy` crate.
pub(crate) fn unified_diff(path: &str, original: &str, modified: &str) -> String {
    let patch = diffy::create_patch(original, modified);
    // diffy's default header uses `---` / `+++` with "original" / "modified".
    // Replace those placeholder paths with the real file path for clarity.
    patch
        .to_string()
        .replace("--- original\n", &format!("--- a/{path}\n"))
        .replace("+++ modified\n", &format!("+++ b/{path}\n"))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::sync::CancellationToken;

    fn ctx() -> ToolContext {
        ToolContext {
            correlation_id: "test".to_string(),
            step_id: 0,
            cancellation: CancellationToken::new(),
            stream_tx: None,
        }
    }

    /// Create a temp file inside the current working directory so it passes
    /// the `PathGuard::from_cwd()` sandbox check.
    fn tmpfile_in_cwd(contents: &str) -> tempfile::NamedTempFile {
        let f = tempfile::Builder::new()
            .tempfile_in(".")
            .expect("create temp file in CWD");
        std::fs::write(f.path(), contents).unwrap();
        f
    }

    // ── EditFileTool ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn edit_file_replaces_once() {
        let tmp = tmpfile_in_cwd("Hello world\n");

        let out = EditFileTool
            .run(
                EditFileArgs {
                    path: tmp.path().to_str().unwrap().to_string(),
                    old_string: "world".to_string(),
                    new_string: "Rust".to_string(),
                    dry_run: false,
                },
                ctx(),
            )
            .await
            .unwrap();

        assert!(out.ok);
        assert_eq!(std::fs::read_to_string(tmp.path()).unwrap(), "Hello Rust\n");
        assert!(out.diff.contains("+Hello Rust"));
    }

    #[tokio::test]
    async fn edit_file_dry_run_does_not_write() {
        let tmp = tmpfile_in_cwd("Hello world\n");

        let out = EditFileTool
            .run(
                EditFileArgs {
                    path: tmp.path().to_str().unwrap().to_string(),
                    old_string: "world".to_string(),
                    new_string: "Rust".to_string(),
                    dry_run: true,
                },
                ctx(),
            )
            .await
            .unwrap();

        assert!(!out.ok);
        assert_eq!(out.bytes_written, 0);
        assert_eq!(std::fs::read_to_string(tmp.path()).unwrap(), "Hello world\n");
        assert!(out.diff.contains("-Hello world"));
        assert!(out.diff.contains("+Hello Rust"));
    }

    #[tokio::test]
    async fn edit_file_rejects_ambiguous_string() {
        let tmp = tmpfile_in_cwd("foo foo\n");

        let err = EditFileTool
            .run(
                EditFileArgs {
                    path: tmp.path().to_str().unwrap().to_string(),
                    old_string: "foo".to_string(),
                    new_string: "bar".to_string(),
                    dry_run: false,
                },
                ctx(),
            )
            .await
            .unwrap_err();

        assert!(err.to_string().contains("2 times"));
    }

    // ── ReplaceLinesTool ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn replace_lines_replaces_range() {
        let tmp = tmpfile_in_cwd("line1\nline2\nline3\nline4\n");

        let out = ReplaceLinesTool
            .run(
                ReplaceLinesArgs {
                    path: tmp.path().to_str().unwrap().to_string(),
                    start_line: 2,
                    end_line: 3,
                    new_content: "replaced".to_string(),
                    dry_run: false,
                },
                ctx(),
            )
            .await
            .unwrap();

        assert!(out.ok);
        assert_eq!(
            std::fs::read_to_string(tmp.path()).unwrap(),
            "line1\nreplaced\nline4\n"
        );
        assert!(out.diff.contains("-line2"));
        assert!(out.diff.contains("+replaced"));
    }

    #[tokio::test]
    async fn replace_lines_dry_run_does_not_write() {
        let tmp = tmpfile_in_cwd("line1\nline2\n");

        let out = ReplaceLinesTool
            .run(
                ReplaceLinesArgs {
                    path: tmp.path().to_str().unwrap().to_string(),
                    start_line: 1,
                    end_line: 1,
                    new_content: "new".to_string(),
                    dry_run: true,
                },
                ctx(),
            )
            .await
            .unwrap();

        assert!(!out.ok);
        assert_eq!(std::fs::read_to_string(tmp.path()).unwrap(), "line1\nline2\n");
        assert!(out.diff.contains("-line1"));
    }
}
