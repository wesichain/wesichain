use wesichain_agent::{Tool, ToolRegistry};
use wesichain_core::{Value, WesichainError};
use wesichain_llm::ToolSpec;

struct EchoTool;

#[async_trait::async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "echoes"
    }

    fn schema(&self) -> Value {
        Value::from("schema")
    }

    async fn call(&self, input: Value) -> Result<Value, WesichainError> {
        Ok(input)
    }
}

#[tokio::test]
async fn registry_calls_tool() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(EchoTool));
    let output = registry.call("echo", Value::from("hi")).await.unwrap();
    assert_eq!(output, Value::from("hi"));

    let specs: Vec<ToolSpec> = registry.to_specs();
    assert_eq!(specs.len(), 1);
}
