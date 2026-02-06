//! Integration tests for Google Gemini provider
//! Run with: cargo test -p wesichain-llm --features google -- --ignored

#![cfg(feature = "google")]

use futures::StreamExt;
use wesichain_core::{Runnable, StreamEvent};
use wesichain_llm::{GoogleClient, LlmRequest, Message, Role};

#[tokio::test]
#[ignore = "Requires GOOGLE_API_KEY environment variable"]
async fn test_google_simple_completion() {
    let api_key = std::env::var("GOOGLE_API_KEY").expect("GOOGLE_API_KEY not set");
    let client = GoogleClient::new(api_key, "gemini-1.5-flash");

    let request = LlmRequest {
        model: "".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "Say 'Hello from Wesichain' and nothing else.".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };

    let response = client.invoke(request).await.expect("Request failed");
    assert!(!response.content.trim().is_empty());
}

#[tokio::test]
#[ignore = "Requires GOOGLE_API_KEY environment variable"]
async fn test_google_streaming_completion() {
    let api_key = std::env::var("GOOGLE_API_KEY").expect("GOOGLE_API_KEY not set");
    let client = GoogleClient::new(api_key, "gemini-1.5-flash");

    let request = LlmRequest {
        model: "".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "Count from 1 to 5.".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };

    let mut stream = client.stream(request);
    let mut chunks = String::new();

    while let Some(event) = stream.next().await {
        match event.expect("stream event") {
            StreamEvent::ContentChunk(chunk) => chunks.push_str(&chunk),
            StreamEvent::FinalAnswer(answer) => {
                if !answer.is_empty() {
                    chunks = answer;
                }
                break;
            }
            _ => {}
        }
    }

    assert!(!chunks.trim().is_empty());
}
