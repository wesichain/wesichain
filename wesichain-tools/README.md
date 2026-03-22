# wesichain-tools

Built-in coding tools for Wesichain agents — filesystem, shell, git, glob, grep, and patch.

Part of the [wesichain](https://github.com/wesichain/wesichain) LLM agent framework.

## Installation

```toml
[dependencies]
wesichain-tools = { version = "0.3", features = ["fs", "exec", "git"] }
```

## Quick Start

```rust
use wesichain_tools::fs::{ReadFileTool, WriteFileTool};
use wesichain_agent::tooling::ToolRegistry;

let mut registry = ToolRegistry::new();
registry.register(ReadFileTool::new());
registry.register(WriteFileTool::new());
// pass registry to a ReAct agent
```

## Feature Flags

| Flag | Tools |
|------|-------|
| `fs` (default) | `ReadFileTool`, `WriteFileTool`, `EditFileTool`, `GlobTool`, `GrepTool`, `PatchTool` |
| `exec` | `BashExecTool` — sandboxed shell execution |
| `git` | `GitStatusTool`, `GitDiffTool`, `GitCommitTool` |
| `http` (default) | `HttpGetTool` |
| `search` | Web search tool |

### PathGuard Sandbox

All filesystem tools accept an optional `PathGuard` that restricts operations to a safe root directory, preventing path traversal attacks.

```rust
use wesichain_tools::fs::PathGuard;
let guard = PathGuard::new("/workspace");
let tool = ReadFileTool::with_guard(guard);
```

## License

Apache-2.0 OR MIT
