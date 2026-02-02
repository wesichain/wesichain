use async_trait::async_trait;
use futures::stream::StreamExt;
use wesichain_agent::{Tool, ToolCallingAgent, ToolRegistry};
use wesichain_core::{Runnable, StreamEvent, Value, WesichainError};
use wesichain_llm::{LlmRequest, LlmResponse, ToolCall};

struct MockLlm;

#[async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockLlm {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        if input.messages.len() == 1 {
            return Ok(LlmResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: "1".to_string(),
                    name: "echo".to_string(),
                    args: Value::from("hi"),
                }],
            });
        }
        Ok(LlmResponse {
            content: "done".to_string(),
            tool_calls: vec![],
        })
    }

    fn stream(
        &self,
        _input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "echo"
    }

    fn schema(&self) -> Value {
        Value::from("schema")
    }

    async fn call(&self, input: Value) -> Result<Value, WesichainError> {
        Ok(input)
    }
}

#[tokio::test]
async fn agent_calls_tool_then_finishes() {
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(EchoTool));

    let agent = ToolCallingAgent::new(MockLlm, tools, "mock".to_string()).max_steps(3);
    let output = agent.invoke("hi".to_string()).await.unwrap();
    assert_eq!(output, "done");
}

#[tokio::test]
async fn agent_stops_after_max_steps() {
    struct LoopLlm;

    #[async_trait]
    impl Runnable<LlmRequest, LlmResponse> for LoopLlm {
        async fn invoke(&self, _input: LlmRequest) -> Result<LlmResponse, WesichainError> {
            Ok(LlmResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: "1".to_string(),
                    name: "echo".to_string(),
                    args: Value::from("hi"),
                }],
            })
        }

        fn stream(
            &self,
            _input: LlmRequest,
        ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
            futures::stream::empty().boxed()
        }
    }

    let mut tools = ToolRegistry::new();
    tools.register(Box::new(EchoTool));

    let agent = ToolCallingAgent::new(LoopLlm, tools, "mock".to_string()).max_steps(2);
    let err = agent.invoke("hi".to_string()).await.unwrap_err();
    assert!(matches!(err, WesichainError::Custom(_)));
}
