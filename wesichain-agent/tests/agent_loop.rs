#![allow(deprecated)]

use async_trait::async_trait;
use futures::stream::StreamExt;
use std::sync::{Arc, Mutex};
use wesichain_agent::{Tool, ToolCallingAgent, ToolRegistry};
use wesichain_core::{Runnable, StreamEvent, ToolError, Value, WesichainError};
use wesichain_llm::{LlmRequest, LlmResponse, Message, Role, ToolCall};

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

    async fn invoke(&self, input: Value) -> Result<Value, ToolError> {
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

#[tokio::test]
async fn agent_includes_assistant_message_before_tool_results() {
    struct RecordingLlm {
        calls: Arc<Mutex<Vec<Vec<Message>>>>,
    }

    #[async_trait]
    impl Runnable<LlmRequest, LlmResponse> for RecordingLlm {
        async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
            self.calls.lock().unwrap().push(input.messages.clone());
            if input.messages.len() == 1 {
                return Ok(LlmResponse {
                    content: "need tool".to_string(),
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

    let calls = Arc::new(Mutex::new(Vec::new()));
    let llm = RecordingLlm {
        calls: Arc::clone(&calls),
    };
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(EchoTool));

    let agent = ToolCallingAgent::new(llm, tools, "mock".to_string()).max_steps(3);
    let output = agent.invoke("hi".to_string()).await.unwrap();
    assert_eq!(output, "done");

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded.len(), 2);
    let second_call = &recorded[1];
    assert!(second_call
        .iter()
        .any(|message| { message.role == Role::Assistant && message.content == "need tool" }));
    assert_eq!(second_call.len(), 3);
}

#[tokio::test]
async fn agent_returns_tool_call_failed_for_missing_tool() {
    struct MissingToolLlm;

    #[async_trait]
    impl Runnable<LlmRequest, LlmResponse> for MissingToolLlm {
        async fn invoke(&self, _input: LlmRequest) -> Result<LlmResponse, WesichainError> {
            Ok(LlmResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: "1".to_string(),
                    name: "missing".to_string(),
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

    let tools = ToolRegistry::new();
    let agent = ToolCallingAgent::new(MissingToolLlm, tools, "mock".to_string()).max_steps(1);
    let err = agent.invoke("hi".to_string()).await.unwrap_err();
    assert!(matches!(err, WesichainError::ToolCallFailed { .. }));
}

#[tokio::test]
async fn agent_stream_returns_not_implemented_error() {
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(EchoTool));

    let agent = ToolCallingAgent::new(MockLlm, tools, "mock".to_string());
    let mut stream = agent.stream("hi".to_string());
    let result = stream.next().await.unwrap();
    assert!(
        matches!(result, Err(WesichainError::Custom(message)) if message == "stream not implemented")
    );
}
