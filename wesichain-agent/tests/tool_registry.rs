#![allow(deprecated)]

use wesichain_agent::{Tool, ToolRegistry};
use wesichain_core::{ToolError, ToolSpec, Value};

struct EchoTool;

struct AlphaTool;

struct ZuluTool;

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

    async fn invoke(&self, input: Value) -> Result<Value, ToolError> {
        Ok(input)
    }
}

#[async_trait::async_trait]
impl Tool for AlphaTool {
    fn name(&self) -> &str {
        "alpha"
    }

    fn description(&self) -> &str {
        "alpha tool"
    }

    fn schema(&self) -> Value {
        Value::from("alpha-schema")
    }

    async fn invoke(&self, input: Value) -> Result<Value, ToolError> {
        Ok(input)
    }
}

#[async_trait::async_trait]
impl Tool for ZuluTool {
    fn name(&self) -> &str {
        "zulu"
    }

    fn description(&self) -> &str {
        "zulu tool"
    }

    fn schema(&self) -> Value {
        Value::from("zulu-schema")
    }

    async fn invoke(&self, input: Value) -> Result<Value, ToolError> {
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
    assert_eq!(specs[0].name, "echo");
    assert_eq!(specs[0].description, "echoes");
    assert_eq!(specs[0].parameters, Value::from("schema"));
}

#[tokio::test]
async fn registry_orders_specs_by_name() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ZuluTool));
    registry.register(Box::new(AlphaTool));

    let specs: Vec<ToolSpec> = registry.to_specs();
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].name, "alpha");
    assert_eq!(specs[0].description, "alpha tool");
    assert_eq!(specs[0].parameters, Value::from("alpha-schema"));
    assert_eq!(specs[1].name, "zulu");
    assert_eq!(specs[1].description, "zulu tool");
    assert_eq!(specs[1].parameters, Value::from("zulu-schema"));
}

#[tokio::test]
async fn registry_reports_missing_tool() {
    let registry = ToolRegistry::new();
    let error = registry
        .call("missing", Value::from("input"))
        .await
        .expect_err("expected missing tool error");

    assert!(matches!(error, ToolError::ExecutionFailed(_)));
    assert!(error.to_string().contains("not found"));
}
