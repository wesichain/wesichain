//! `wesichain` — project scaffolding CLI + interactive agent REPL.
//!
//! Commands:
//! - `wesichain new <name>` — create a new wesichain project
//! - `wesichain run` — start an interactive agent REPL
//! - `wesichain add tool <name>` — scaffold a new #[tool] function
//! - `wesichain add provider <name>` — add a provider to Cargo.toml + example

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Command::New { name } => cmd_new(&name),
        Command::Init => cmd_init(),
        Command::Check => cmd_check(),
        Command::Run { endpoint, model, session, token } => {
            cmd_run(&endpoint, &model, session.as_deref(), token.as_deref()).await
        }
        Command::Diff { original, modified, apply } => {
            cmd_diff(&original, &modified, apply)
        }
        Command::Add { resource } => match resource {
            AddResource::Tool { name } => cmd_add_tool(&name),
            AddResource::Provider { name } => cmd_add_provider(&name),
            AddResource::Mcp { server } => cmd_add_mcp(&server),
        },
    }
}

// ── CLI schema ────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "wesichain", about = "Wesichain project scaffolding", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new wesichain project
    New { name: String },
    /// Scaffold a wesichain project in the current directory
    Init,
    /// Run `cargo check --all-features` in the current project
    Check,
    /// Start an interactive agent REPL connected to a wesichain server
    Run {
        /// Server endpoint URL
        #[arg(long, env = "WESICHAIN_ENDPOINT", default_value = "http://localhost:3000")]
        endpoint: String,
        /// Model to use
        #[arg(long, env = "WESICHAIN_MODEL", default_value = "claude-3-5-haiku-20241022")]
        model: String,
        /// Resume a previous session by ID
        #[arg(long, env = "WESICHAIN_SESSION")]
        session: Option<String>,
        /// Bearer token for server auth
        #[arg(long, env = "WESICHAIN_TOKEN")]
        token: Option<String>,
    },
    /// Show a colored unified diff between two files, with optional apply prompt
    Diff {
        /// Original file
        original: PathBuf,
        /// Modified file to compare against original
        modified: PathBuf,
        /// Apply the diff (copy modified over original) after confirmation
        #[arg(short, long)]
        apply: bool,
    },
    /// Add resources to an existing project
    Add {
        #[command(subcommand)]
        resource: AddResource,
    },
}

#[derive(Subcommand)]
enum AddResource {
    /// Scaffold a new #[tool] function
    Tool { name: String },
    /// Add a provider (openai, anthropic, mistral, azure, google, ollama)
    Provider { name: String },
    /// Show instructions for adding an MCP server
    Mcp { server: String },
}

// ── wesichain run (interactive REPL) ─────────────────────────────────────────

use futures::StreamExt;
use reqwest::Client;
use rustyline::history::FileHistory;
use rustyline::{Config, Editor};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    stream: bool,
}

#[derive(Serialize, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    content: String,
    #[serde(default)]
    thinking: String,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    session_id: String,
}

async fn cmd_run(
    endpoint: &str,
    model: &str,
    session: Option<&str>,
    token: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Set up history file
    let history_path = dirs::home_dir()
        .map(|h| h.join(".wesichain").join("history"))
        .unwrap_or_else(|| PathBuf::from(".wesichain_history"));

    if let Some(parent) = history_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let rl_config = Config::builder()
        .history_ignore_space(true)
        .max_history_size(1000)?
        .build();

    let mut rl: Editor<(), FileHistory> = Editor::with_config(rl_config)?;
    let _ = rl.load_history(&history_path);

    // Build HTTP client
    let client = Client::new();
    let mut messages: Vec<ChatMessage> = Vec::new();
    let mut current_session = session.map(String::from);

    println!("Wesichain agent REPL — model: {model}");
    println!("  endpoint: {endpoint}");
    if let Some(sid) = &current_session {
        println!("  session: {sid}");
    }
    println!("  type 'exit' or Ctrl-D to quit\n");

    loop {
        let input = match rl.readline("> ") {
            Ok(line) => line.trim().to_string(),
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("\nBye!");
                break;
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("^C (type 'exit' or Ctrl-D to quit)");
                continue;
            }
            Err(e) => return Err(e.into()),
        };

        if input.is_empty() {
            continue;
        }

        rl.add_history_entry(&input)?;

        if input == "exit" || input == "quit" {
            println!("Bye!");
            break;
        }

        messages.push(ChatMessage { role: "user".to_string(), content: input.clone() });

        let req_body = ChatRequest {
            model: model.to_string(),
            messages: messages.clone(),
            session_id: current_session.clone(),
            stream: true,
        };

        let url = format!("{endpoint}/v1/chat");

        let mut req = client.post(&url).json(&req_body);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }

        let response = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("connection error: {e}");
                messages.pop(); // remove the user message we just added
                continue;
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            eprintln!("server error {status}: {body}");
            messages.pop();
            continue;
        }

        // Stream the response
        let mut stream = response.bytes_stream();
        let mut assistant_reply = String::new();
        let mut in_thinking = false;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let text = String::from_utf8_lossy(&chunk);

            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line == "data: [DONE]" {
                    continue;
                }
                let data = if let Some(rest) = line.strip_prefix("data: ") {
                    rest
                } else {
                    line
                };

                if let Ok(sc) = serde_json::from_str::<StreamChunk>(data) {
                    if !sc.session_id.is_empty() && current_session.is_none() {
                        current_session = Some(sc.session_id.clone());
                    }

                    if !sc.thinking.is_empty() {
                        if !in_thinking {
                            print!("\x1b[2m<thinking> ");
                            in_thinking = true;
                        }
                        print!("{}", sc.thinking);
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                    } else if in_thinking && !sc.content.is_empty() {
                        println!(" </thinking>\x1b[0m");
                        in_thinking = false;
                        print!("{}", sc.content);
                        assistant_reply.push_str(&sc.content);
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                    } else if !sc.content.is_empty() {
                        print!("{}", sc.content);
                        assistant_reply.push_str(&sc.content);
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                    }

                    if sc.done {
                        if in_thinking {
                            println!(" </thinking>\x1b[0m");
                            in_thinking = false;
                        }
                        println!();
                        if let Some(sid) = &current_session {
                            println!("\x1b[2m[session: {sid}]\x1b[0m");
                        }
                    }
                }
            }
        }

        if in_thinking {
            println!(" </thinking>\x1b[0m");
        }

        if !assistant_reply.is_empty() {
            messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: assistant_reply,
            });
        }
    }

    let _ = rl.save_history(&history_path);
    Ok(())
}

// ── wesichain diff <original> <modified> ─────────────────────────────────────

fn cmd_diff(
    original: &PathBuf,
    modified: &PathBuf,
    apply: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // ANSI colour codes
    const RED: &str = "\x1b[31m";
    const GREEN: &str = "\x1b[32m";
    const CYAN: &str = "\x1b[36m";
    const DIM: &str = "\x1b[2m";
    const RESET: &str = "\x1b[0m";

    let orig_text = std::fs::read_to_string(original)
        .map_err(|e| format!("Cannot read {:?}: {e}", original))?;
    let mod_text = std::fs::read_to_string(modified)
        .map_err(|e| format!("Cannot read {:?}: {e}", modified))?;

    let patch = diffy::create_patch(&orig_text, &mod_text);
    let patch_str = patch.to_string();

    if patch_str.trim().is_empty() {
        println!("Files are identical.");
        return Ok(());
    }

    // Print colorised unified diff
    for line in patch_str.lines() {
        if line.starts_with("---") || line.starts_with("+++") {
            println!("{CYAN}{line}{RESET}");
        } else if line.starts_with("@@") {
            println!("{DIM}{line}{RESET}");
        } else if line.starts_with('+') {
            println!("{GREEN}{line}{RESET}");
        } else if line.starts_with('-') {
            println!("{RED}{line}{RESET}");
        } else {
            println!("{line}");
        }
    }

    if !apply {
        return Ok(());
    }

    // Interactive y/N/e prompt
    print!("\nApply diff? [y/N/e(edit)] ");
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => {
            std::fs::copy(modified, original)?;
            println!("Applied — {:?} updated.", original);
        }
        "e" | "edit" => {
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            std::process::Command::new(&editor).arg(modified).status()?;
            println!("Opened {:?} in {editor}. Re-run diff to apply.", modified);
        }
        _ => {
            println!("Skipped.");
        }
    }

    Ok(())
}

// ── wesichain new <name> ──────────────────────────────────────────────────────

fn cmd_new(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(name);
    if dir.exists() {
        return Err(format!("directory '{name}' already exists").into());
    }

    std::fs::create_dir_all(dir.join("src"))?;

    // Cargo.toml
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
wesichain = "0.2"
tokio     = {{ version = "1", features = ["full"] }}
anyhow    = "1"
"#
        ),
    )?;

    // src/main.rs
    std::fs::write(
        dir.join("src/main.rs"),
        r#"use wesichain::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Replace with your API key or read from env
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("Set ANTHROPIC_API_KEY");

    let llm = wesichain::llm::AnthropicClient::new(api_key, "claude-3-5-haiku-20241022");

    let req = LlmRequest {
        model: "claude-3-5-haiku-20241022".to_string(),
        messages: vec![Message::user("Hello from wesichain!")],
        tools: vec![],
        temperature: None,
        max_tokens: Some(256),
        stop_sequences: vec![],
    };

    let resp = llm.invoke(req).await?;
    println!("{}", resp.content);

    Ok(())
}
"#,
    )?;

    println!("Created project '{name}'");
    println!("  cd {name} && cargo run");
    Ok(())
}

// ── wesichain add tool <name> ─────────────────────────────────────────────────

fn cmd_add_tool(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let snake = to_snake_case(name);
    let pascal = to_pascal_case(name);
    let path = PathBuf::from(format!("src/tools/{snake}.rs"));

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if path.exists() {
        return Err(format!("{} already exists", path.display()).into());
    }

    std::fs::write(
        &path,
        format!(
            r#"use schemars::JsonSchema;
use serde::{{Deserialize, Serialize}};
use wesichain::prelude::{{ToolContext, ToolError, TypedTool}};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct {pascal}Args {{
    /// Describe your argument here
    pub input: String,
}}

#[derive(Debug, Serialize, JsonSchema)]
pub struct {pascal}Output {{
    pub result: String,
}}

#[derive(Clone, Default)]
pub struct {pascal}Tool;

#[async_trait::async_trait]
impl TypedTool for {pascal}Tool {{
    type Args = {pascal}Args;
    type Output = {pascal}Output;
    const NAME: &'static str = "{snake}";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {{
        // TODO: implement tool logic
        Ok({pascal}Output {{
            result: format!("processed: {{}}", args.input),
        }})
    }}
}}
"#
        ),
    )?;

    println!("Created tool scaffold at {}", path.display());
    println!("Register it with: ToolSet::new().register_with({pascal}Tool::default())");
    Ok(())
}

// ── wesichain add provider <name> ─────────────────────────────────────────────

fn cmd_add_provider(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (feature, example) = match name.to_lowercase().as_str() {
        "openai" => (
            r#"wesichain-llm = { version = "0.2", features = ["openai"] }"#,
            r#"use wesichain_llm::providers::openai::OpenAiClient;
let llm = OpenAiClient::new(std::env::var("OPENAI_API_KEY").unwrap());"#,
        ),
        "anthropic" => (
            r#"wesichain-anthropic = "0.2""#,
            r#"use wesichain_anthropic::AnthropicClient;
let llm = AnthropicClient::new(std::env::var("ANTHROPIC_API_KEY").unwrap(), "claude-3-5-haiku-20241022");"#,
        ),
        "mistral" => (
            r#"wesichain-llm = { version = "0.2", features = ["mistral"] }"#,
            r#"use wesichain_llm::providers::mistral::MistralClient;
let llm = MistralClient::new(std::env::var("MISTRAL_API_KEY").unwrap(), "mistral-large-latest");"#,
        ),
        "azure" => (
            r#"wesichain-llm = { version = "0.2", features = ["azure"] }"#,
            r#"use wesichain_llm::providers::azure::AzureOpenAiClient;
let llm = AzureOpenAiClient::new("my-resource", "my-deployment", std::env::var("AZURE_OPENAI_KEY").unwrap())?;"#,
        ),
        "google" => (
            r#"wesichain-llm = { version = "0.2", features = ["google"] }"#,
            r#"use wesichain_llm::providers::google::GoogleClient;
let llm = GoogleClient::new(std::env::var("GOOGLE_API_KEY").unwrap(), "gemini-1.5-flash");"#,
        ),
        "ollama" => (
            r#"wesichain-llm = "0.2"  # ollama is default feature"#,
            r#"use wesichain_llm::OllamaClient;
let llm = OllamaClient::new("http://localhost:11434", "llama3.2");"#,
        ),
        "groq" => (
            r#"wesichain-llm = { version = "0.2", features = ["groq"] }"#,
            r#"use wesichain_llm::providers::groq::GroqClient;
let llm = GroqClient::new(std::env::var("GROQ_API_KEY").unwrap(), "llama-3.3-70b-versatile");"#,
        ),
        "together" => (
            r#"wesichain-llm = { version = "0.2", features = ["together"] }"#,
            r#"use wesichain_llm::providers::together::TogetherClient;
let llm = TogetherClient::new(std::env::var("TOGETHER_API_KEY").unwrap(), "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo");"#,
        ),
        other => {
            return Err(format!(
                "unknown provider '{other}'. Valid: openai, anthropic, mistral, azure, google, ollama, groq, together"
            )
            .into());
        }
    };

    println!("Add to Cargo.toml:\n  {feature}\n");
    println!("Usage:\n  {example}");
    Ok(())
}

// ── wesichain init ────────────────────────────────────────────────────────────

fn cmd_init() -> Result<(), Box<dyn std::error::Error>> {
    // Write .env.example with API key stubs
    let env_example = r#"# Wesichain API keys — copy to .env and fill in your values
ANTHROPIC_API_KEY=
OPENAI_API_KEY=
AZURE_OPENAI_KEY=
AZURE_OPENAI_RESOURCE=
AZURE_OPENAI_DEPLOYMENT=
GOOGLE_API_KEY=
MISTRAL_API_KEY=
TAVILY_API_KEY=
"#;
    if PathBuf::from(".env.example").exists() {
        println!(".env.example already exists — skipping");
    } else {
        std::fs::write(".env.example", env_example)?;
        println!("Created .env.example");
    }

    println!("Project initialised. Next steps:");
    println!("  cp .env.example .env  # fill in your API keys");
    println!("  cargo build");
    Ok(())
}

// ── wesichain check ───────────────────────────────────────────────────────────

fn cmd_check() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running: cargo check --all-features");
    let status = std::process::Command::new("cargo")
        .args(["check", "--all-features"])
        .status()?;
    if !status.success() {
        return Err("cargo check failed".into());
    }
    println!("All good!");
    Ok(())
}

// ── wesichain add mcp <server> ────────────────────────────────────────────────

fn cmd_add_mcp(server: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("To add the MCP server '{server}':\n");
    println!("1. Add wesichain-mcp to your Cargo.toml:");
    println!("   wesichain-mcp = {{ path = \"path/to/wesichain-mcp\", version = \"0.2\" }}");
    println!();
    println!("2. Use it in code:");
    println!("   use wesichain_mcp::bridge::ToolSetBuilderMcpExt;");
    println!("   use wesichain_agent::ToolSet;");
    println!();
    println!("   let tools = ToolSet::new()");
    println!("       .add_mcp_server(\"{server}\", &[/* args */])");
    println!("       .await?");
    println!("       .build()?;");
    Ok(())
}

// ── string helpers ────────────────────────────────────────────────────────────

fn to_snake_case(s: &str) -> String {
    s.chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_uppercase() && i > 0 {
                format!("_{}", c.to_lowercase())
            } else {
                c.to_lowercase().to_string()
            }
        })
        .collect::<String>()
        .replace('-', "_")
}

fn to_pascal_case(s: &str) -> String {
    s.split(['_', '-'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}
