//! `GlobTool` — gitignore-aware file pattern matching.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wesichain_core::{ToolContext, ToolError, TypedTool};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GlobArgs {
    /// Glob pattern relative to `base_dir` (e.g. `"src/**/*.rs"`).
    /// Must not contain `..`.
    pub pattern: String,
    /// Base directory to search in (defaults to CWD)
    pub base_dir: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GlobOutput {
    pub matches: Vec<String>,
}

#[derive(Clone, Default)]
pub struct GlobTool;

#[async_trait::async_trait]
impl TypedTool for GlobTool {
    type Args = GlobArgs;
    type Output = GlobOutput;
    const NAME: &'static str = "glob";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        let base = args
            .base_dir
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        if args.pattern.contains("..") {
            return Err(ToolError::InvalidInput(
                "glob pattern must not contain '..'".to_string(),
            ));
        }

        let pattern = glob::Pattern::new(&args.pattern)
            .map_err(|e| ToolError::InvalidInput(format!("invalid glob pattern: {e}")))?;

        let base_canonical = base.canonicalize().unwrap_or_else(|_| base.clone());

        let mut matches = Vec::new();

        // Walk the tree respecting .gitignore (and .git/ itself).
        let walker = ignore::WalkBuilder::new(&base).git_ignore(true).build();

        for entry in walker.flatten() {
            // Skip the root entry itself (depth 0).
            if entry.depth() == 0 {
                continue;
            }

            // Match the relative path against the glob pattern.
            if let Ok(relative) = entry.path().strip_prefix(&base) {
                if pattern.matches_path(relative) {
                    // Confine to base directory (defense-in-depth).
                    if let Ok(canonical) = entry.path().canonicalize() {
                        if canonical.starts_with(&base_canonical) {
                            matches.push(entry.path().to_string_lossy().into_owned());
                        }
                    } else {
                        matches.push(entry.path().to_string_lossy().into_owned());
                    }
                }
            }
        }

        Ok(GlobOutput { matches })
    }
}
