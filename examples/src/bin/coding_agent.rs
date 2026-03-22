//! Full coding agent example — demonstrates the complete wesichain coding-agent stack.
//!
//! This agent can:
//! - Read, write, edit, and patch files
//! - Search for code with glob and grep
//! - Run bash commands with real-time streaming output
//! - Check git status and diffs
//! - Detect the workspace type (Rust/Python/Node)
//! - Resume from a persisted session across runs
//! - Track token usage and cost
//!
//! # Usage
//! ```bash
//! ANTHROPIC_API_KEY=sk-ant-... cargo run --bin coding_agent
//! ```
//!
//! Set `WESICHAIN_SESSION=<id>` to resume a previous session.
//! Set `WESICHAIN_WORKSPACE=<path>` to target a specific directory (display only).

use futures::StreamExt;
use wesichain_agent::{CancellationToken, ToolCallEnvelope, ToolContext, ToolSet};
use wesichain_anthropic::AnthropicClient;
use wesichain_core::{
    capability::for_model as caps_for, token_budget::TokenBudget, LlmRequest, Message, Role,
    StreamEvent,
};
use wesichain_session::{FileSessionStore, Session, SessionCostSummary, SessionStore};
use wesichain_tools::{ToolBundle, WorkspaceDetectorTool};

// ── Configuration ─────────────────────────────────────────────────────────────

const MODEL: &str = "claude-3-5-sonnet-20241022";
const MAX_TOKENS: u32 = 8192;
const TOKEN_BUDGET: usize = 180_000;

// ── System prompt ─────────────────────────────────────────────────────────────

fn system_prompt(workspace_info: &str) -> String {
    format!(
        "You are a skilled software engineer with access to tools for reading, \
writing, editing, and searching code. You work carefully and precisely.\n\n\
Workspace: {workspace_info}\n\n\
Guidelines:\n\
- Always read a file before editing it\n\
- Use grep to find code before trying to understand it\n\
- Prefer targeted edits (edit_file / replace_lines) over full rewrites\n\
- Run tests with bash after making changes\n\
- Keep the user informed of your progress\n\
- Ask for clarification when requirements are ambiguous"
    )
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");

    let workspace_root =
        std::env::var("WESICHAIN_WORKSPACE").unwrap_or_else(|_| ".".to_string());
    let session_id = std::env::var("WESICHAIN_SESSION").ok();

    // ── Capability check ─────────────────────────────────────────────────────
    let caps = caps_for(MODEL);
    println!("Model: {MODEL}");
    println!("  tools={} vision={} thinking={}", caps.tools, caps.vision, caps.thinking);
    println!("  context={} tokens", caps.context_window);

    // ── Workspace detection ───────────────────────────────────────────────────
    let workspace_info = detect_workspace(&workspace_root);
    println!("Workspace: {workspace_info}");

    // ── Tool registration ─────────────────────────────────────────────────────
    let tools: ToolSet = ToolBundle::coding_tools()
        .register_with(WorkspaceDetectorTool::default())
        .build()
        .expect("tool registration failed");
    println!("Tools registered: {}", tools.names().len());

    // ── LLM client ───────────────────────────────────────────────────────────
    let mut llm = AnthropicClient::new(api_key, MODEL);
    if caps.thinking {
        llm = llm.with_thinking(8192);
    }

    // ── Session setup ─────────────────────────────────────────────────────────
    let store = FileSessionStore::new(".wesichain/sessions");
    let (mut session, resumed) = if let Some(ref sid) = session_id {
        match store.load(sid).await? {
            Some(s) => {
                println!("Resuming session {sid} ({} messages)", s.messages.len());
                (s, true)
            }
            None => (Session::new(sid.clone()), false),
        }
    } else {
        (Session::new(uuid::Uuid::new_v4().to_string()), false)
    };

    // ── Token budget ──────────────────────────────────────────────────────────
    let budget = TokenBudget::new(TOKEN_BUDGET);

    // ── Cost tracking ─────────────────────────────────────────────────────────
    let mut cost_summary = SessionCostSummary::default();

    // ── Build initial message history ─────────────────────────────────────────
    let sys_msg = system_prompt(&workspace_info);
    let mut messages: Vec<Message> = if resumed {
        session.messages.clone()
    } else {
        vec![Message::system(sys_msg.as_str())]
    };

    // ── Interactive loop ──────────────────────────────────────────────────────
    println!("\nCoding agent ready. Type your request (Ctrl-D to quit).\n");

    let cancellation = CancellationToken::new();

    loop {
        let user_input = read_line("> ")?;
        if user_input.is_empty() {
            break;
        }

        messages.push(Message::user(user_input.as_str()));

        // Trim to token budget before each call
        budget.apply(&mut messages);

        // ── Agentic loop: keep going until no more tool calls ─────────────────
        loop {
            let request = LlmRequest {
                model: MODEL.to_string(),
                messages: messages.clone(),
                tools: tools.tool_specs(),
                temperature: None,
                max_tokens: Some(MAX_TOKENS),
                stop_sequences: vec![],
            };

            // Stream the response
            use wesichain_core::Runnable;
            let mut stream = llm.stream(request);

            let mut assistant_reply = String::new();
            let mut thinking_buf = String::new();
            let mut in_thinking = false;
            let mut pending_tool_calls = vec![];

            while let Some(event) = stream.next().await {
                match event? {
                    StreamEvent::ThinkingChunk(t) => {
                        if !in_thinking {
                            print!("\x1b[2m<thinking>");
                            in_thinking = true;
                        }
                        print!("{t}");
                        thinking_buf.push_str(&t);
                        flush();
                    }
                    StreamEvent::ContentChunk(c) => {
                        if in_thinking {
                            println!("</thinking>\x1b[0m");
                            in_thinking = false;
                        }
                        print!("{c}");
                        assistant_reply.push_str(&c);
                        flush();
                    }
                    StreamEvent::ToolCallStart { name, id } => {
                        println!("\n\x1b[33m[tool] {name} ...\x1b[0m");
                        pending_tool_calls.push((id, name));
                    }
                    StreamEvent::FinalAnswer(_) => {
                        println!();
                        break;
                    }
                    _ => {}
                }
            }

            if in_thinking {
                println!("</thinking>\x1b[0m");
            }
            let _ = thinking_buf;

            // Append assistant turn
            if !assistant_reply.is_empty() {
                messages.push(Message::assistant(assistant_reply.as_str()));
            }

            // If no tool calls, we're done with this user turn
            if pending_tool_calls.is_empty() {
                break;
            }

            // Re-invoke (non-streaming) to get structured tool call data
            let req = LlmRequest {
                model: MODEL.to_string(),
                messages: messages.clone(),
                tools: tools.tool_specs(),
                temperature: None,
                max_tokens: Some(MAX_TOKENS),
                stop_sequences: vec![],
            };
            let resp = llm.invoke(req).await?;
            cost_summary.add(&resp);

            if resp.tool_calls.is_empty() {
                break;
            }

            // Build envelopes and dispatch
            let envelopes: Vec<ToolCallEnvelope> = resp
                .tool_calls
                .iter()
                .map(|tc| ToolCallEnvelope {
                    name: tc.name.clone(),
                    args: tc.args.clone(),
                    call_id: tc.id.clone(),
                })
                .collect();

            let ctx = ToolContext::new(
                session.id.clone(),
                messages.len() as u32,
                cancellation.clone(),
            );
            let results = tools.dispatch_many(envelopes, ctx).await;

            // Append tool result messages
            for (call_id, result) in results {
                let content = match result {
                    Ok(v) => v.to_string(),
                    Err(e) => format!("error: {e}"),
                };
                println!("\x1b[2m  → {}\x1b[0m", truncate(&content, 120));
                messages.push(Message {
                    role: Role::Tool,
                    content: content.into(),
                    tool_call_id: Some(call_id),
                    tool_calls: vec![],
                });
            }
        }

        // Persist session
        session.messages = messages.clone();
        store.save(&session).await?;
        println!(
            "\x1b[2m[session: {} | cost: ${:.4} total | {} messages]\x1b[0m",
            session.id,
            cost_summary.total_cost_usd,
            messages.len()
        );
    }

    println!(
        "\nSession complete.\n  Tokens: {}p / {}c\n  Cost:   ${:.4}",
        cost_summary.total_prompt_tokens,
        cost_summary.total_completion_tokens,
        cost_summary.total_cost_usd
    );

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn detect_workspace(root: &str) -> String {
    let path = std::path::Path::new(root);
    if path.join("Cargo.toml").exists() {
        format!("Rust project at {root}")
    } else if path.join("package.json").exists() {
        format!("Node.js project at {root}")
    } else if path.join("pyproject.toml").exists() || path.join("setup.py").exists() {
        format!("Python project at {root}")
    } else {
        format!("Directory: {root}")
    }
}

fn read_line(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    use std::io::Write;
    print!("{prompt}");
    std::io::stdout().flush()?;
    let mut line = String::new();
    let n = std::io::stdin().read_line(&mut line)?;
    if n == 0 {
        return Ok(String::new()); // EOF
    }
    Ok(line.trim().to_string())
}

fn flush() {
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
