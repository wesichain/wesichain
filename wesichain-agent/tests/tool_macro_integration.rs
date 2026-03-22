use wesichain_agent::{ToolCallEnvelope, ToolContext, ToolSet};
use wesichain_core::CancellationToken;
use wesichain_macros::tool;

/// Add two numbers
#[tool(description = "Add two numbers together")]
async fn add_numbers(a: i64, b: i64) -> Result<i64, String> {
    Ok(a + b)
}

#[tokio::test]
async fn tool_macro_generates_typed_tool() {
    let tool_set = ToolSet::new().register_with(AddNumbersTool).build().unwrap();

    assert!(tool_set.names().contains(&"add_numbers"));

    let envelope = ToolCallEnvelope {
        name: "add_numbers".to_string(),
        args: serde_json::json!({ "a": 3, "b": 4 }),
        call_id: "test-1".to_string(),
    };

    let ctx = ToolContext {
        correlation_id: "test-corr".to_string(),
        step_id: 0,
        cancellation: CancellationToken::new(),
        stream_tx: None,
    };

    let result = tool_set.dispatch(envelope, ctx).await.unwrap();
    assert_eq!(result, serde_json::json!(7i64));
}
