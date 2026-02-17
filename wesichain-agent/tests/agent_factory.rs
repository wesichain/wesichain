use async_trait::async_trait;
use futures::StreamExt;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use wesichain_agent::{create_tool_calling_agent, AgentStep};
use wesichain_core::{
    LlmRequest, LlmResponse, Message, Role, Runnable, Tool, ToolCall, ToolCallingLlm, ToolError,
    WesichainError,
};
use wesichain_prompt::ChatPromptTemplate;

// Mock LLM that returns a tool call or a final answer based on configuration
#[derive(Clone)]
struct MockAgentLlm {
    response_sequence: Arc<Mutex<Vec<LlmResponse>>>,
}

#[async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockAgentLlm {
    async fn invoke(&self, _request: LlmRequest) -> Result<LlmResponse, WesichainError> {
        let mut sequence = self.response_sequence.lock().unwrap();
        if sequence.is_empty() {
            panic!("MockAgentLlm ran out of responses");
        }
        Ok(sequence.remove(0))
    }

    fn stream(
        &self,
        _input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

impl ToolCallingLlm for MockAgentLlm {}

// Mock Tool
struct CalculatorTool;
#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }
    fn description(&self) -> &str {
        "math"
    }
    fn schema(&self) -> Value {
        json!({})
    }
    async fn invoke(&self, _args: Value) -> Result<Value, ToolError> {
        Ok(json!("42"))
    }
}

#[tokio::test]
async fn test_create_tool_calling_agent_action() {
    let tool_call_response = LlmResponse {
        content: "".to_string(),
        tool_calls: vec![ToolCall {
            id: "call1".to_string(),
            name: "calculator".to_string(),
            args: json!({"a": 1, "b": 2}),
        }],
    };

    let llm = MockAgentLlm {
        response_sequence: Arc::new(Mutex::new(vec![tool_call_response])),
    };

    let tools: Vec<Box<dyn Tool>> = vec![Box::new(CalculatorTool)];
    let prompt = ChatPromptTemplate::new(vec![]);

    let agent = create_tool_calling_agent(Box::new(llm), tools, prompt);

    let request = LlmRequest {
        model: "test-model".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "Calculate 1 + 2".to_string(),
            tool_calls: vec![],
            tool_call_id: None,
        }],
        tools: vec![], // Logic should inject tools
    };

    let step = agent.invoke(request).await.unwrap();

    match step {
        AgentStep::Action(action) => {
            assert_eq!(action.tool, "calculator");
            assert_eq!(action.tool_input, json!({"a": 1, "b": 2}));
        }
        _ => panic!("Expected AgentStep::Action"),
    }
}

#[tokio::test]
async fn test_create_tool_calling_agent_finish() {
    let final_response = LlmResponse {
        content: "The answer is 42".to_string(),
        tool_calls: vec![],
    };

    let llm = MockAgentLlm {
        response_sequence: Arc::new(Mutex::new(vec![final_response])),
    };

    let tools: Vec<Box<dyn Tool>> = vec![Box::new(CalculatorTool)];
    let prompt = ChatPromptTemplate::new(vec![]);

    let agent = create_tool_calling_agent(Box::new(llm), tools, prompt);

    let request = LlmRequest {
        model: "test-model".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "What is the answer?".to_string(),
            tool_calls: vec![],
            tool_call_id: None,
        }],
        tools: vec![],
    };

    let step = agent.invoke(request).await.unwrap();

    match step {
        AgentStep::Finish(finish) => {
            assert_eq!(finish.return_values, json!({"output": "The answer is 42"}));
        }
        _ => panic!("Expected AgentStep::Finish"),
    }
}
