#![allow(deprecated)]
use std::sync::Arc;

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wesichain_core::{
    HasFinalOutput, HasUserInput, LlmRequest, LlmResponse, ReActStep, Runnable, ScratchpadState,
    Tool, ToolCall, ToolCallingLlm, ToolError, Value, WesichainError,
};
use wesichain_graph::{ExecutionOptions, GraphBuilder, GraphState, ReActAgentNode, StateSchema};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    input: String,
    scratchpad: Vec<ReActStep>,
    final_output: Option<String>,
    iterations: u32,
}

impl StateSchema for DemoState {
    type Update = Self;
    fn apply(_: &Self, update: Self) -> Self {
        update
    }
}

impl ScratchpadState for DemoState {
    fn scratchpad(&self) -> &Vec<ReActStep> {
        &self.scratchpad
    }

    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> {
        &mut self.scratchpad
    }

    fn iteration_count(&self) -> u32 {
        self.iterations
    }

    fn increment_iteration(&mut self) {
        self.iterations += 1;
    }
}

impl HasUserInput for DemoState {
    fn user_input(&self) -> &str {
        &self.input
    }
}

impl HasFinalOutput for DemoState {
    fn final_output(&self) -> Option<&str> {
        self.final_output.as_deref()
    }

    fn set_final_output(&mut self, value: String) {
        self.final_output = Some(value);
    }
}

struct MockTool;

#[async_trait::async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "math"
    }

    fn schema(&self) -> Value {
        json!({"type": "object"})
    }

    async fn invoke(&self, _args: Value) -> Result<Value, ToolError> {
        Ok(json!(4))
    }
}

struct MockLlm;

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockLlm {
    async fn invoke(&self, _request: LlmRequest) -> Result<LlmResponse, WesichainError> {
        Ok(LlmResponse {
            content: "".to_string(),
            tool_calls: vec![ToolCall {
                id: "c1".to_string(),
                name: "calculator".to_string(),
                args: json!({"expression": "2+2"}),
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

impl ToolCallingLlm for MockLlm {}

#[tokio::test]
async fn react_agent_executes_tool_and_finishes() {
    let llm = Arc::new(MockLlm);
    let tool = Arc::new(MockTool);
    let node = ReActAgentNode::builder()
        .llm(llm)
        .tools(vec![tool])
        .build()
        .unwrap();

    let graph = GraphBuilder::new()
        .add_node("agent", node)
        .set_entry("agent")
        .build();
    let state = GraphState::new(DemoState {
        input: "2+2".to_string(),
        ..Default::default()
    });
    let out = graph
        .invoke_with_options(state, ExecutionOptions::default())
        .await
        .unwrap();
    assert_eq!(out.data.final_output.as_deref(), Some(""));
    assert!(out
        .data
        .scratchpad
        .iter()
        .any(|step| matches!(step, ReActStep::Observation(_))));
}
