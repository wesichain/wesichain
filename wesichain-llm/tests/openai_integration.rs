//! Integration tests for OpenAI provider
//! Run with: cargo test --features openai -- --ignored

#![cfg(feature = "openai")]

use wesichain_core::Runnable;
use wesichain_llm::{LlmRequest, Message, OpenAiClient, Role};

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY environment variable"]
async fn test_openai_simple_completion() {
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let client = OpenAiClient::new(api_key);

    let request = LlmRequest {
        model: "gpt-4o-mini".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "Say 'Hello from Wesichain'".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };

    let response = client.invoke(request).await.expect("Request failed");
    assert!(response.content.contains("Hello") || response.content.contains("Wesichain"));
}
