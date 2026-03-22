# wesichain-cli

Project scaffolding CLI and interactive REPL for Wesichain agents.

Part of the [wesichain](https://github.com/wesichain/wesichain) LLM agent framework.

## Installation

```bash
cargo install wesichain-cli
```

Or add as a dependency:

```toml
[dependencies]
wesichain-cli = "0.3"
```

## Commands

```bash
# Create a new wesichain project
wesichain new my-agent

# Run a coding agent interactively
wesichain run

# Run against a specific endpoint
wesichain run --endpoint http://localhost:8080
```

## Features

- **`wesichain new`** — scaffold a new Rust project with agent, tools, and optional MCP server
- **`wesichain run`** — interactive REPL powered by `rustyline` with readline history
- **ANSI diff viewer** — inline colored diffs when the agent edits files
- **Streaming output** — real-time token streaming from the agent
- **Config loading** — reads `~/.wesichain/config.toml` for API keys and defaults

## License

Apache-2.0 OR MIT
