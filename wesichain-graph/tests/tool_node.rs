use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use wesichain_agent::Tool;
use wesichain_core::{Value, WesichainError};
use wesichain_graph::{GraphState, HasToolCalls, StateSchema, ToolNode};
use wesichain_llm::{Message, Role, ToolCall};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct AgentState {
    tool_calls: Vec<ToolCall>,
    tool_results: Vec<Message>,
}

impl StateSchema for AgentState {}

impl HasToolCalls for AgentState {
    fn tool_calls(&self) -> &Vec<ToolCall> {
        &self.tool_calls
    }

    fn push_tool_result(&mut self, message: Message) {
        self.tool_results.push(message);
    }
}

#[derive(Default)]
struct MockTool {
    calls: Arc<Mutex<Vec<Value>>>,
}

#[async_trait::async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "echo"
    }

    fn schema(&self) -> Value {
        serde_json::json!({"type": "object"})
    }

    async fn call(&self, input: Value) -> Result<Value, WesichainError> {
        self.calls.lock().unwrap().push(input.clone());
        Ok(input)
    }
}

#[tokio::test]
async fn tool_node_executes_calls_and_appends_results() {
    let calls = vec![ToolCall {
        id: "1".into(),
        name: "echo".into(),
        args: serde_json::json!({"text": "hi"}),
    }];
    let state = GraphState::new(AgentState {
        tool_calls: calls,
        tool_results: Vec::new(),
    });
    let tool = Arc::new(MockTool::default());
    let calls_log = tool.calls.clone();
    let node = ToolNode::new(vec![tool]);
    let update = node.invoke(state).await.unwrap();
    assert_eq!(calls_log.lock().unwrap().len(), 1);
    assert_eq!(update.data.tool_results.len(), 1);
    assert_eq!(update.data.tool_results[0].role, Role::Tool);
}
