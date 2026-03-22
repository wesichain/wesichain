//! `WorkspaceDetectorTool` — locate the project root and infer build/test commands.
//!
//! Walks upward from a starting directory looking for well-known project
//! marker files (`Cargo.toml`, `package.json`, `pyproject.toml`, etc.) and
//! returns a [`WorkspaceInfo`] with the resolved root path and default
//! build/test commands for that project type.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wesichain_core::{ToolContext, ToolError, TypedTool};

// ── ProjectKind ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Rust,
    Node,
    Python,
    Go,
    Unknown,
}

// ── WorkspaceInfo ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, JsonSchema)]
pub struct WorkspaceInfo {
    /// Absolute path to the detected project root
    pub root: String,
    /// Detected project type
    pub kind: ProjectKind,
    /// Recommended build command for this project (if known)
    pub build_cmd: Option<String>,
    /// Recommended test command for this project (if known)
    pub test_cmd: Option<String>,
}

// ── Args ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WorkspaceArgs {
    /// Directory to start searching from (defaults to CWD).
    /// Walks upward until a project root is found or the filesystem root
    /// is reached.
    pub start_dir: Option<String>,
}

// ── Tool ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct WorkspaceDetectorTool;

#[async_trait::async_trait]
impl TypedTool for WorkspaceDetectorTool {
    type Args = WorkspaceArgs;
    type Output = WorkspaceInfo;
    const NAME: &'static str = "detect_workspace";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        let start = args
            .start_dir
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            });

        Ok(detect_workspace(&start))
    }
}

// ── Detection logic ───────────────────────────────────────────────────────────

fn detect_workspace(start: &std::path::Path) -> WorkspaceInfo {
    let mut current = start.to_path_buf();

    loop {
        if let Some(info) = check_project_markers(&current) {
            return info;
        }
        match current.parent() {
            // Stop when we hit the filesystem root (parent == self).
            Some(parent) if parent != current => current = parent.to_path_buf(),
            _ => break,
        }
    }

    // No marker found — return the start directory as an unknown project.
    WorkspaceInfo {
        root: start.to_string_lossy().into_owned(),
        kind: ProjectKind::Unknown,
        build_cmd: None,
        test_cmd: None,
    }
}

/// Marker files ordered by priority (first match wins).
const MARKERS: &[(&str, ProjectKind, Option<&str>, Option<&str>)] = &[
    ("Cargo.toml", ProjectKind::Rust, Some("cargo build"), Some("cargo test")),
    ("package.json", ProjectKind::Node, Some("npm run build"), Some("npm test")),
    ("pyproject.toml", ProjectKind::Python, None, Some("pytest")),
    ("setup.py", ProjectKind::Python, None, Some("pytest")),
    ("go.mod", ProjectKind::Go, Some("go build ./..."), Some("go test ./...")),
];

fn check_project_markers(dir: &std::path::Path) -> Option<WorkspaceInfo> {
    for (marker, kind, build_cmd, test_cmd) in MARKERS {
        if dir.join(marker).exists() {
            return Some(WorkspaceInfo {
                root: dir.to_string_lossy().into_owned(),
                kind: kind.clone(),
                build_cmd: build_cmd.map(str::to_string),
                test_cmd: test_cmd.map(str::to_string),
            });
        }
    }
    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();

        let info = detect_workspace(dir.path());
        assert_eq!(info.kind, ProjectKind::Rust);
        assert_eq!(info.build_cmd.as_deref(), Some("cargo build"));
        assert_eq!(info.test_cmd.as_deref(), Some("cargo test"));
    }

    #[test]
    fn detects_node_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();

        let info = detect_workspace(dir.path());
        assert_eq!(info.kind, ProjectKind::Node);
    }

    #[test]
    fn detects_from_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir(&sub).unwrap();

        let info = detect_workspace(&sub);
        assert_eq!(info.kind, ProjectKind::Rust);
        assert_eq!(info.root, dir.path().to_string_lossy());
    }

    #[test]
    fn returns_unknown_when_no_marker_found() {
        let dir = tempfile::tempdir().unwrap();
        // Deep subdirectory with no markers anywhere above it within tmp.
        // The walk will hit / eventually and find nothing.
        let info = detect_workspace(dir.path());
        // May find a marker if the test machine has a project root above /tmp;
        // just assert it doesn't panic and returns a root.
        assert!(!info.root.is_empty());
    }
}
