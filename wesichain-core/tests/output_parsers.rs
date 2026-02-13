use serde_json::{json, Value};
use wesichain_core::{JsonOutputParser, LlmResponse, Runnable, StrOutputParser};

#[tokio::test]
async fn test_str_output_parser() {
    let parser = StrOutputParser;

    // Test with String input
    let input = "Hello world".to_string();
    let output = parser.invoke(input).await.unwrap();
    assert_eq!(output, "Hello world");

    // Test with LlmResponse input
    let response = LlmResponse {
        content: "Hello from LLM".to_string(),
        tool_calls: vec![],
    };
    let output = parser.invoke(response).await.unwrap();
    assert_eq!(output, "Hello from LLM");
}

#[tokio::test]
async fn test_json_output_parser() {
    let parser = JsonOutputParser::<Value>::new();

    // Test with valid JSON string
    let json_str = r#"{"key": "value"}"#;
    let output = parser.invoke(json_str.to_string()).await.unwrap();
    assert_eq!(output, json!({"key": "value"}));

    // Test with markdown code block
    let markdown_json = r#"```json
{
    "key": "value"
}
```"#;
    let output = parser.invoke(markdown_json.to_string()).await.unwrap();
    assert_eq!(output, json!({"key": "value"}));

    // Test with LlmResponse
    let response = LlmResponse {
        content: markdown_json.to_string(),
        tool_calls: vec![],
    };
    let output = parser.invoke(response).await.unwrap();
    assert_eq!(output, json!({"key": "value"}));
}

#[tokio::test]
async fn test_json_output_parser_typed() {
    #[derive(serde::Deserialize, serde::Serialize, PartialEq, Debug)]
    struct MyStruct {
        foo: String,
        bar: i32,
    }

    let parser = JsonOutputParser::<MyStruct>::new();
    let json_str = r#"{"foo": "baz", "bar": 42}"#;
    let output = Runnable::<String, MyStruct>::invoke(&parser, json_str.to_string())
        .await
        .unwrap();

    assert_eq!(
        output,
        MyStruct {
            foo: "baz".to_string(),
            bar: 42
        }
    );
}
