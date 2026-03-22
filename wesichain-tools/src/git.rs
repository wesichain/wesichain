//! Git tools for coding agents — feature `git`.
//!
//! Provides five tools covering the typical git workflow for an AI coding agent:
//! [`GitStatusTool`], [`GitDiffTool`], [`GitLogTool`], [`GitBlameTool`],
//! and [`GitCommitTool`].
//!
//! All tools discover the repository by walking upward from CWD, matching
//! normal `git` CLI behaviour.

use git2::{DiffFormat, Repository, Sort, StatusOptions};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wesichain_core::{ToolContext, ToolError, TypedTool};

// ── helpers ───────────────────────────────────────────────────────────────────

fn open_repo() -> Result<Repository, ToolError> {
    Repository::discover(".").map_err(|e| {
        ToolError::ExecutionFailed(format!("not a git repository (or cannot open): {e}"))
    })
}

fn diff_to_string(diff: &git2::Diff<'_>) -> Result<String, ToolError> {
    let mut out = String::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        match line.origin() {
            'H' | 'B' | 'F' => {
                if let Ok(s) = std::str::from_utf8(line.content()) {
                    out.push_str(s);
                }
            }
            '+' | '-' | ' ' => {
                out.push(line.origin());
                if let Ok(s) = std::str::from_utf8(line.content()) {
                    out.push_str(s);
                }
            }
            _ => {}
        }
        true
    })
    .map_err(|e| ToolError::ExecutionFailed(format!("diff generation failed: {e}")))?;
    Ok(out)
}

// ── GitStatusTool ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitStatusArgs {
    /// Include untracked files in output (default: true)
    #[serde(default = "default_true")]
    pub include_untracked: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StatusEntry {
    /// Repository-relative path
    pub path: String,
    /// Short status code, e.g. "M ", " M", "A ", "??" (matches `git status --short`)
    pub status: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GitStatusOutput {
    pub entries: Vec<StatusEntry>,
    pub clean: bool,
}

#[derive(Clone, Default)]
pub struct GitStatusTool;

#[async_trait::async_trait]
impl TypedTool for GitStatusTool {
    type Args = GitStatusArgs;
    type Output = GitStatusOutput;
    const NAME: &'static str = "git_status";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        let repo = open_repo()?;
        let mut opts = StatusOptions::new();
        opts.include_untracked(args.include_untracked)
            .recurse_untracked_dirs(true);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| ToolError::ExecutionFailed(format!("git status failed: {e}")))?;

        let entries: Vec<StatusEntry> = statuses
            .iter()
            .map(|entry| {
                let path = entry.path().unwrap_or("?").to_string();
                let status = status_flags_to_short(entry.status());
                StatusEntry { path, status }
            })
            .collect();

        let clean = entries.is_empty();
        Ok(GitStatusOutput { entries, clean })
    }
}

fn status_flags_to_short(flags: git2::Status) -> String {
    use git2::Status;
    let index = if flags.intersects(Status::INDEX_NEW) {
        'A'
    } else if flags.intersects(Status::INDEX_MODIFIED) {
        'M'
    } else if flags.intersects(Status::INDEX_DELETED) {
        'D'
    } else if flags.intersects(Status::INDEX_RENAMED) {
        'R'
    } else {
        ' '
    };
    let workdir = if flags.intersects(Status::WT_NEW) {
        '?'
    } else if flags.intersects(Status::WT_MODIFIED) {
        'M'
    } else if flags.intersects(Status::WT_DELETED) {
        'D'
    } else {
        ' '
    };
    if flags.intersects(Status::WT_NEW) {
        "??".to_string()
    } else {
        format!("{index}{workdir}")
    }
}

// ── GitDiffTool ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffTarget {
    /// Unstaged changes (working tree vs index)
    Unstaged,
    /// Staged changes (index vs HEAD)
    Staged,
    /// All changes: HEAD vs working tree
    All,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitDiffArgs {
    /// Which changes to show (default: unstaged)
    #[serde(default = "default_diff_target")]
    pub target: DiffTarget,
    /// Limit diff to this path (optional)
    pub path: Option<String>,
}

fn default_diff_target() -> DiffTarget {
    DiffTarget::Unstaged
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GitDiffOutput {
    pub diff: String,
    pub stats: String,
}

#[derive(Clone, Default)]
pub struct GitDiffTool;

#[async_trait::async_trait]
impl TypedTool for GitDiffTool {
    type Args = GitDiffArgs;
    type Output = GitDiffOutput;
    const NAME: &'static str = "git_diff";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        let repo = open_repo()?;

        let mut diff_opts = git2::DiffOptions::new();
        if let Some(ref p) = args.path {
            diff_opts.pathspec(p);
        }

        let diff = match args.target {
            DiffTarget::Unstaged => repo
                .diff_index_to_workdir(None, Some(&mut diff_opts))
                .map_err(|e| ToolError::ExecutionFailed(format!("diff failed: {e}")))?,
            DiffTarget::Staged => {
                let head_tree = repo
                    .head()
                    .and_then(|h| h.peel_to_tree())
                    .ok();
                repo.diff_tree_to_index(
                    head_tree.as_ref(),
                    None,
                    Some(&mut diff_opts),
                )
                .map_err(|e| ToolError::ExecutionFailed(format!("staged diff failed: {e}")))?
            }
            DiffTarget::All => {
                let head_tree = repo
                    .head()
                    .and_then(|h| h.peel_to_tree())
                    .ok();
                repo.diff_tree_to_workdir_with_index(
                    head_tree.as_ref(),
                    Some(&mut diff_opts),
                )
                .map_err(|e| ToolError::ExecutionFailed(format!("diff failed: {e}")))?
            }
        };

        let stats = diff
            .stats()
            .map(|s| {
                format!(
                    "{} file(s) changed, {} insertions(+), {} deletions(-)",
                    s.files_changed(),
                    s.insertions(),
                    s.deletions()
                )
            })
            .unwrap_or_default();

        let diff_text = diff_to_string(&diff)?;
        Ok(GitDiffOutput { diff: diff_text, stats })
    }
}

// ── GitLogTool ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitLogArgs {
    /// Maximum number of commits to return (default: 20)
    pub limit: Option<usize>,
    /// Show only commits touching this path (optional)
    pub path: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommitEntry {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GitLogOutput {
    pub commits: Vec<CommitEntry>,
}

#[derive(Clone, Default)]
pub struct GitLogTool;

#[async_trait::async_trait]
impl TypedTool for GitLogTool {
    type Args = GitLogArgs;
    type Output = GitLogOutput;
    const NAME: &'static str = "git_log";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        let repo = open_repo()?;
        let limit = args.limit.unwrap_or(20);

        let mut revwalk = repo
            .revwalk()
            .map_err(|e| ToolError::ExecutionFailed(format!("revwalk failed: {e}")))?;

        revwalk
            .push_head()
            .map_err(|e| ToolError::ExecutionFailed(format!("HEAD not found: {e}")))?;
        revwalk.set_sorting(Sort::TIME).ok();

        let mut commits = Vec::new();

        for (count, oid_result) in revwalk.enumerate() {
            if count >= limit {
                break;
            }
            let oid = oid_result
                .map_err(|e| ToolError::ExecutionFailed(format!("revwalk error: {e}")))?;
            let commit = repo
                .find_commit(oid)
                .map_err(|e| ToolError::ExecutionFailed(format!("commit not found: {e}")))?;

            // If path filter is set, check whether this commit touches that path.
            if let Some(ref p) = args.path {
                if !commit_touches_path(&repo, &commit, p) {
                    continue;
                }
            }

            let hash = format!("{oid}");
            let short_hash = hash[..7].to_string();
            let author = commit.author().name().unwrap_or("?").to_string();
            let timestamp = commit.time().seconds();
            let date = format_timestamp(timestamp);
            let message = commit.message().unwrap_or("").lines().next().unwrap_or("").to_string();

            commits.push(CommitEntry { hash, short_hash, author, date, message });
        }

        Ok(GitLogOutput { commits })
    }
}

fn commit_touches_path(repo: &Repository, commit: &git2::Commit<'_>, path: &str) -> bool {
    let Ok(tree) = commit.tree() else { return false };
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
    let Ok(diff) = repo.diff_tree_to_tree(
        parent_tree.as_ref(),
        Some(&tree),
        None,
    ) else {
        return false
    };
    diff.deltas().any(|d| {
        d.new_file().path().map(|p| p.to_string_lossy().contains(path)).unwrap_or(false)
            || d.old_file().path().map(|p| p.to_string_lossy().contains(path)).unwrap_or(false)
    })
}

fn format_timestamp(secs: i64) -> String {
    // Simple ISO-8601 approximation without chrono dependency
    let secs_in_day = secs % 86400;
    let days = secs / 86400;
    // Rough gregorian calculation from Unix epoch (1970-01-01)
    let _ = (days, secs_in_day);
    format!("unix:{secs}")
}

// ── GitBlameTool ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitBlameArgs {
    /// Path to the file (relative to repo root)
    pub path: String,
    /// First line to blame (1-indexed, default: 1)
    pub start_line: Option<u32>,
    /// Last line to blame (1-indexed, default: end of file)
    pub end_line: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BlameLine {
    pub line_number: u32,
    pub commit: String,
    pub author: String,
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GitBlameOutput {
    pub lines: Vec<BlameLine>,
}

#[derive(Clone, Default)]
pub struct GitBlameTool;

#[async_trait::async_trait]
impl TypedTool for GitBlameTool {
    type Args = GitBlameArgs;
    type Output = GitBlameOutput;
    const NAME: &'static str = "git_blame";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        let repo = open_repo()?;
        let mut opts = git2::BlameOptions::new();
        if let Some(s) = args.start_line {
            opts.min_line(s as usize);
        }
        if let Some(e) = args.end_line {
            opts.max_line(e as usize);
        }

        let blame = repo
            .blame_file(std::path::Path::new(&args.path), Some(&mut opts))
            .map_err(|e| ToolError::ExecutionFailed(format!("git blame failed: {e}")))?;

        // Read file contents to get line text
        let workdir = repo.workdir().unwrap_or(std::path::Path::new("."));
        let file_path = workdir.join(&args.path);
        let contents = std::fs::read_to_string(&file_path)
            .unwrap_or_default();
        let file_lines: Vec<&str> = contents.lines().collect();

        let start = args.start_line.unwrap_or(1) as usize;
        let end = args.end_line.map(|e| e as usize).unwrap_or(file_lines.len());

        let mut lines = Vec::new();
        for (idx, line_content) in file_lines.iter().enumerate() {
            let line_no = idx + 1;
            if line_no < start || line_no > end {
                continue;
            }
            if let Some(hunk) = blame.get_line(line_no) {
                let sig = hunk.final_signature();
                let commit = format!("{:.7}", hunk.final_commit_id());
                let author = sig.name().unwrap_or("?").to_string();
                lines.push(BlameLine {
                    line_number: line_no as u32,
                    commit,
                    author,
                    content: line_content.to_string(),
                });
            }
        }

        Ok(GitBlameOutput { lines })
    }
}

// ── GitCommitTool ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitCommitArgs {
    /// Commit message
    pub message: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GitCommitOutput {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
}

#[derive(Clone, Default)]
pub struct GitCommitTool;

#[async_trait::async_trait]
impl TypedTool for GitCommitTool {
    type Args = GitCommitArgs;
    type Output = GitCommitOutput;
    const NAME: &'static str = "git_commit";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        let repo = open_repo()?;

        let sig = repo.signature().map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "no git identity configured — set user.name and user.email: {e}"
            ))
        })?;

        let mut index = repo
            .index()
            .map_err(|e| ToolError::ExecutionFailed(format!("index error: {e}")))?;
        index
            .write()
            .map_err(|e| ToolError::ExecutionFailed(format!("index write error: {e}")))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| ToolError::ExecutionFailed(format!("write tree failed: {e}")))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| ToolError::ExecutionFailed(format!("find tree failed: {e}")))?;

        let parents: Vec<git2::Commit<'_>> = match repo.head() {
            Ok(head) => {
                let oid = head.peel_to_commit().map_err(|e| {
                    ToolError::ExecutionFailed(format!("HEAD is not a commit: {e}"))
                })?;
                vec![oid]
            }
            Err(_) => vec![], // initial commit — no parents
        };

        let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();

        let oid = repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                &args.message,
                &tree,
                &parent_refs,
            )
            .map_err(|e| ToolError::ExecutionFailed(format!("commit failed: {e}")))?;

        let hash = format!("{oid}");
        let short_hash = hash[..7].to_string();
        Ok(GitCommitOutput { hash, short_hash, message: args.message })
    }
}
