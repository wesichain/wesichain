//! Axum SSE streaming server example.
//!
//! Requires: ANTHROPIC_API_KEY environment variable.
//!
//! Run with: cargo run --bin streaming_server
//!
//! Then POST to http://localhost:3000/chat with a JSON body:
//! { "message": "Hello, how are you?" }

use futures::StreamExt;
use wesichain_anthropic::AnthropicClient;
use wesichain_core::{Message, Role, Runnable, StreamEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable must be set");

    let llm = AnthropicClient::new(api_key, "claude-3-5-sonnet-20241022");

    // Demo: stream a response to stdout (SSE server requires axum feature)
    let request = wesichain_core::LlmRequest {
        model: String::new(),
        messages: vec![Message {
            role: Role::User,
            content: "Tell me a short story about a Rust programmer.".into(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        temperature: Some(0.8),
        max_tokens: Some(512),
        stop_sequences: vec![],
    };

    println!("Streaming response (SSE format):\n");

    let mut stream = llm.stream(request);
    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::ContentChunk(text) => {
                println!("data: {}", serde_json::json!({ "content": text }));
            }
            StreamEvent::FinalAnswer(_) => {
                println!("\nevent: done");
                println!("data: {{}}");
            }
            _ => {}
        }
    }

    Ok(())
}
