//! `GrepTool` — gitignore-aware regex search over file contents.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wesichain_core::{ToolContext, ToolError, TypedTool};

const DEFAULT_MAX_RESULTS: usize = 50;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GrepArgs {
    /// Regular expression pattern
    pub pattern: String,
    /// File or directory to search in (relative to CWD or absolute).
    /// Must not contain `..`.
    pub path: String,
    /// Case-sensitive search (default: true)
    #[serde(default = "default_true")]
    pub case_sensitive: bool,
    /// Maximum number of results to return (default: 50)
    pub max_results: Option<usize>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GrepMatch {
    pub file: String,
    pub line: u32,
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GrepOutput {
    pub matches: Vec<GrepMatch>,
    pub truncated: bool,
}

#[derive(Clone, Default)]
pub struct GrepTool;

#[async_trait::async_trait]
impl TypedTool for GrepTool {
    type Args = GrepArgs;
    type Output = GrepOutput;
    const NAME: &'static str = "grep";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        if args.path.contains("..") {
            return Err(ToolError::InvalidInput(
                "path must not contain '..'".to_string(),
            ));
        }

        let max = args.max_results.unwrap_or(DEFAULT_MAX_RESULTS);
        let regex = {
            let mut builder = regex::RegexBuilder::new(&args.pattern);
            builder.case_insensitive(!args.case_sensitive);
            builder
                .build()
                .map_err(|e| ToolError::InvalidInput(format!("invalid regex: {e}")))?
        };

        let search_path = std::path::Path::new(&args.path);
        let mut matches = Vec::new();
        let mut truncated = false;

        // Walk the tree respecting .gitignore — skips target/, node_modules/, etc.
        let walker = ignore::WalkBuilder::new(search_path).git_ignore(true).build();

        for entry in walker.flatten() {
            if truncated {
                break;
            }
            if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                search_file(entry.path(), &regex, &mut matches, max, &mut truncated);
            }
        }

        Ok(GrepOutput { matches, truncated })
    }
}

fn search_file(
    path: &std::path::Path,
    regex: &regex::Regex,
    matches: &mut Vec<GrepMatch>,
    max: usize,
    truncated: &mut bool,
) {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    let file_str = path.to_string_lossy().to_string();

    for (line_idx, line) in contents.lines().enumerate() {
        if regex.is_match(line) {
            if matches.len() >= max {
                *truncated = true;
                return;
            }
            matches.push(GrepMatch {
                file: file_str.clone(),
                line: (line_idx + 1) as u32,
                content: line.to_string(),
            });
        }
    }
}
