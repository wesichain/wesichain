#![allow(deprecated)]

use std::sync::{Arc, Mutex};

use futures::StreamExt;
use wesichain_agent::{Tool, ToolCallingAgent, ToolRegistry};
use wesichain_core::{CallbackHandler, CallbackManager, RunConfig, RunContext, RunType};
use wesichain_core::{Runnable, ToolError, Value, WesichainError};
use wesichain_llm::{LlmRequest, LlmResponse, Role, ToolCall};

struct RecordingHandler {
    starts: Arc<Mutex<Vec<RunType>>>,
    ends: Arc<Mutex<Vec<RunType>>>,
    errors: Arc<Mutex<Vec<RunType>>>,
}

#[async_trait::async_trait]
impl CallbackHandler for RecordingHandler {
    async fn on_start(&self, ctx: &RunContext, _inputs: &Value) {
        self.starts.lock().unwrap().push(ctx.run_type.clone());
    }

    async fn on_end(&self, ctx: &RunContext, _outputs: &Value, _duration_ms: u128) {
        self.ends.lock().unwrap().push(ctx.run_type.clone());
    }

    async fn on_error(&self, ctx: &RunContext, _error: &Value, _duration_ms: u128) {
        self.errors.lock().unwrap().push(ctx.run_type.clone());
    }
}

struct MockLlm;

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockLlm {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        let saw_tool = input.messages.iter().any(|msg| msg.role == Role::Tool);
        if saw_tool {
            return Ok(LlmResponse {
                content: "done".to_string(),
                tool_calls: vec![],
            });
        }

        Ok(LlmResponse {
            content: "".to_string(),
            tool_calls: vec![ToolCall {
                id: "tool-1".to_string(),
                name: "mock".to_string(),
                args: Value::String("hello".to_string()),
            }],
        })
    }

    fn stream(
        &self,
        _input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

struct MockTool;

#[async_trait::async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        "mock"
    }

    fn description(&self) -> &str {
        "mock"
    }

    fn schema(&self) -> Value {
        Value::Null
    }

    async fn invoke(&self, _input: Value) -> Result<Value, ToolError> {
        Ok(Value::String("ok".to_string()))
    }
}

#[tokio::test]
async fn agent_emits_callbacks_for_llm_and_tool() {
    let starts = Arc::new(Mutex::new(Vec::new()));
    let ends = Arc::new(Mutex::new(Vec::new()));
    let errors = Arc::new(Mutex::new(Vec::new()));
    let handler = Arc::new(RecordingHandler {
        starts: starts.clone(),
        ends: ends.clone(),
        errors: errors.clone(),
    });
    let callbacks = CallbackManager::new(vec![handler]);

    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MockTool));

    let agent = ToolCallingAgent::new(MockLlm, registry, "model".to_string())
        .with_run_config(RunConfig {
            callbacks: Some(callbacks),
            ..Default::default()
        })
        .max_steps(2);

    let _ = agent.invoke("input".to_string()).await.unwrap();

    let starts = starts.lock().unwrap().clone();
    let ends = ends.lock().unwrap().clone();
    let errors = errors.lock().unwrap().clone();

    assert!(starts.contains(&RunType::Agent));
    assert!(starts.contains(&RunType::Llm));
    assert!(starts.contains(&RunType::Tool));
    assert!(ends.contains(&RunType::Agent));
    assert!(ends.contains(&RunType::Llm));
    assert!(ends.contains(&RunType::Tool));
    assert!(errors.is_empty());
}
