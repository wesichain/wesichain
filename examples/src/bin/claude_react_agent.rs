//! Real Anthropic ReAct agent example.
//!
//! Requires: ANTHROPIC_API_KEY environment variable.
//!
//! Run with: cargo run --bin claude_react_agent

use futures::StreamExt;
use wesichain_anthropic::AnthropicClient;
use wesichain_core::StreamEvent;
use wesichain_macros::tool;

/// Search the web for information (mock implementation)
#[tool(description = "Search the web for information about a topic")]
async fn search_web(query: String) -> Result<String, String> {
    // In a real implementation, this would call a search API
    Ok(format!("Search results for '{}': [example result]", query))
}

/// Calculate a mathematical expression (mock implementation)
#[tool(description = "Evaluate a mathematical expression")]
async fn calculate(expression: String) -> Result<String, String> {
    // In a real implementation, this would evaluate the expression
    Ok(format!("Result of '{}': 42", expression))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable must be set");

    let llm = AnthropicClient::new(api_key, "claude-3-5-sonnet-20241022");

    println!("Starting ReAct agent with Anthropic Claude...");

    use wesichain_core::Runnable;
    let mut stream = llm.stream(wesichain_core::LlmRequest {
        model: String::new(),
        messages: vec![wesichain_core::Message::user(
            "What is 123 * 456? Also search for information about Rust programming.",
        )],
        tools: vec![],
        temperature: Some(0.7),
        max_tokens: Some(1024),
        stop_sequences: vec![],
    });

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::ContentChunk(text) => print!("{}", text),
            StreamEvent::FinalAnswer(answer) => {
                println!("\n\nFinal answer: {}", answer);
            }
            StreamEvent::ToolCallStart { name, .. } => {
                println!("\n[Tool call: {}]", name);
            }
            _ => {}
        }
    }

    println!("\nDone!");
    Ok(())
}
