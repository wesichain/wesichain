use serde_json::json;
use wesichain_core::{Bindable, LlmRequest, Tool, Value};
use wesichain_macros::tool;

// Define a test tool using the macro
#[tool(name = "calculator", description = "Adds two numbers")]
async fn add(a: i32, b: i32) -> Result<i32, String> {
    Ok(a + b)
}

#[tokio::test]
async fn tool_macro_generates_correct_struct() {
    let tool = ADDTool;
    assert_eq!(tool.name(), "calculator");
    assert_eq!(tool.description(), "Adds two numbers");

    let schema = tool.schema();
    let props = schema.get("properties").unwrap().as_object().unwrap();
    assert!(props.contains_key("a"));
    assert!(props.contains_key("b"));

    let args = json!({ "a": 5, "b": 3 });
    let result: Value = tool.invoke(args).await.unwrap();
    assert_eq!(result.as_i64().unwrap(), 8);
}

#[tokio::test]
async fn bind_works_on_llm_request() {
    let mut req = LlmRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
        tools: vec![],
    };

    let tool_spec = json!({
        "tools": [{
            "name": "test_tool",
            "description": "A test tool",
            "parameters": {}
        }]
    });

    req.bind(tool_spec).unwrap();
    assert_eq!(req.tools.len(), 1);
    assert_eq!(req.tools[0].name, "test_tool");
}
