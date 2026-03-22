//! Per-tool permission policy for the agent runtime.
//!
//! Attach a [`PermissionPolicy`] to a [`ToolSet`] to control which tools the
//! agent can invoke automatically and which require explicit human approval.
//!
//! # Example
//! ```ignore
//! use wesichain_agent::permission::{PermissionPolicy, ToolPermission};
//!
//! let policy = PermissionPolicy::new(ToolPermission::AutoApprove)
//!     .with_tool("bash_exec", ToolPermission::AlwaysAsk)
//!     .with_tool("write_file", ToolPermission::SessionApprove);
//! ```

use std::collections::{HashMap, HashSet};

/// Controls whether a specific tool requires human approval before running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolPermission {
    /// Always run without asking.
    AutoApprove,
    /// Always ask the human before running (even if approved before).
    AlwaysAsk,
    /// Ask once per session; remember the answer.
    SessionApprove,
    /// Never allow this tool to run.
    Never,
}

/// Decision returned by [`PermissionPolicy::check`].
#[derive(Debug, Clone)]
pub enum PermissionCheck {
    /// The tool is allowed to run immediately.
    Allowed,
    /// The tool is blocked.
    Denied,
    /// The caller should request human approval before proceeding.
    NeedsApproval { tool_name: String },
}

/// Policy governing which tools require approval.
#[derive(Clone)]
pub struct PermissionPolicy {
    default: ToolPermission,
    overrides: HashMap<String, ToolPermission>,
    session_approved: HashSet<String>,
}

impl PermissionPolicy {
    pub fn new(default: ToolPermission) -> Self {
        Self {
            default,
            overrides: HashMap::new(),
            session_approved: HashSet::new(),
        }
    }

    /// Override the permission for a specific tool by name.
    pub fn with_tool(mut self, name: &str, perm: ToolPermission) -> Self {
        self.overrides.insert(name.to_string(), perm);
        self
    }

    /// Check whether `tool_name` is allowed to run.
    pub fn check(&mut self, tool_name: &str) -> PermissionCheck {
        let perm = self.overrides.get(tool_name).unwrap_or(&self.default);
        match perm {
            ToolPermission::AutoApprove => PermissionCheck::Allowed,
            ToolPermission::Never => PermissionCheck::Denied,
            ToolPermission::AlwaysAsk => {
                PermissionCheck::NeedsApproval { tool_name: tool_name.to_string() }
            }
            ToolPermission::SessionApprove => {
                if self.session_approved.contains(tool_name) {
                    PermissionCheck::Allowed
                } else {
                    PermissionCheck::NeedsApproval { tool_name: tool_name.to_string() }
                }
            }
        }
    }

    /// Mark a tool as approved for the remainder of this session.
    pub fn record_session_approval(&mut self, tool_name: &str) {
        self.session_approved.insert(tool_name.to_string());
    }
}
