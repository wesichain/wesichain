//! `PatchTool` — apply a unified diff to a file.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wesichain_core::{ToolContext, ToolError, TypedTool};

use crate::file_system::validate_path;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PatchArgs {
    /// Path to the file to patch (relative to CWD or absolute)
    pub path: String,
    /// Unified diff string (as produced by `diff -u` or `git diff`)
    pub patch: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PatchOutput {
    pub ok: bool,
    pub bytes_written: usize,
}

#[derive(Clone, Default)]
pub struct PatchTool;

#[async_trait::async_trait]
impl TypedTool for PatchTool {
    type Args = PatchArgs;
    type Output = PatchOutput;
    const NAME: &'static str = "patch_file";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        validate_path(&args.path)?;
        let original = std::fs::read_to_string(&args.path)?;
        let patched = diffy::apply(&original, &diffy::Patch::from_str(&args.patch)
            .map_err(|e| ToolError::InvalidInput(format!("invalid patch: {e}")))?)
            .map_err(|e| ToolError::ExecutionFailed(format!("patch failed: {e}")))?;
        let bytes = patched.len();
        std::fs::write(&args.path, &patched)?;
        Ok(PatchOutput { ok: true, bytes_written: bytes })
    }
}
