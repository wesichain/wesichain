use std::sync::Arc;
use std::time::Duration;

use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_core::{
    HasFinalOutput, HasUserInput, LlmRequest, LlmResponse, ReActStep, ScratchpadState, Tool,
    ToolCall, ToolCallingLlm, ToolError, Value, WesichainError,
};
use wesichain_graph::{ExecutionOptions, GraphBuilder, GraphState, ReActAgentNode, StateSchema};
use wesichain_langsmith::{LangSmithConfig, LangSmithObserver};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    input: String,
    scratchpad: Vec<ReActStep>,
    final_output: Option<String>,
    iterations: u32,
}

impl StateSchema for DemoState {}

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
impl ToolCallingLlm for MockLlm {
    async fn invoke(&self, _request: LlmRequest) -> Result<LlmResponse, WesichainError> {
        Ok(LlmResponse {
            content: "".to_string(),
            tool_calls: vec![ToolCall {
                id: Uuid::new_v4().to_string(),
                name: "calculator".to_string(),
                args: json!({"expression": "2+2"}),
            }],
        })
    }
}

#[tokio::test]
async fn langsmith_traces_react_agent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex("/runs"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path_regex("/runs/.*"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let config = LangSmithConfig {
        api_key: SecretString::new("key".to_string()),
        api_url: server.uri(),
        project_name: "test".to_string(),
        flush_interval: Duration::from_secs(3600),
        max_batch_size: 50,
        queue_capacity: 1000,
        sampling_rate: 1.0,
        redact_regex: None,
    };

    let observer = Arc::new(LangSmithObserver::new(config));
    let options = ExecutionOptions {
        observer: Some(observer.clone()),
        ..Default::default()
    };

    let node = ReActAgentNode::builder()
        .llm(Arc::new(MockLlm))
        .tools(vec![Arc::new(MockTool)])
        .max_iterations(1)
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

    let _ = graph.invoke_with_options(state, options).await.unwrap();
    let stats = observer.flush(Duration::from_secs(5)).await.unwrap();

    assert!(stats.runs_flushed > 0);
    let requests = server.received_requests().await.unwrap();
    assert!(requests.iter().any(|req| req.method == "POST"));
    assert!(requests.iter().any(|req| req.method == "PATCH"));

    let has_parent = requests.iter().any(|req| {
        if req.method != "POST" {
            return false;
        }
        let payload: serde_json::Value = match serde_json::from_slice(&req.body) {
            Ok(value) => value,
            Err(_) => return false,
        };
        payload
            .get("parent_run_id")
            .map(|value| !value.is_null())
            .unwrap_or(false)
    });
    assert!(has_parent);
}
