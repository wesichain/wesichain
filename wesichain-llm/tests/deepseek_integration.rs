//! Integration tests for DeepSeek provider
//! Run with: cargo test --features deepseek -- --ignored

#![cfg(feature = "deepseek")]

use wesichain_core::Runnable;
use wesichain_llm::{DeepSeekClient, LlmRequest, Message, Role};

#[tokio::test]
#[ignore = "Requires DEEPSEEK_API_KEY environment variable"]
async fn test_deepseek_simple_completion() {
    let api_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY not set");
    let client = DeepSeekClient::new(api_key);

    let request = LlmRequest {
        model: "deepseek-chat".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "Say 'Hello from DeepSeek'".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };

    let response = client.invoke(request).await.expect("Request failed");
    assert!(response.content.contains("Hello") || response.content.contains("DeepSeek"));
}
