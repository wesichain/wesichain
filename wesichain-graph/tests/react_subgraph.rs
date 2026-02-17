
use std::sync::{Arc, Mutex};
use wesichain_core::{
    HasFinalOutput, HasUserInput, LlmRequest, LlmResponse, ReActStep,
    ScratchpadState, Tool, ToolCallingLlm, WesichainError, Value, Runnable, ToolError, StreamEvent
};
use wesichain_graph::{
    GraphState, StateSchema, ReActGraphBuilder,
};
use serde::{Deserialize, Serialize};
use futures::stream::{self, BoxStream, StreamExt};

// --- Mock State ---
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct MockState {
    input: String,
    scratchpad: Vec<ReActStep>,
    final_output: Option<String>,
    iteration_count: u32,
}

impl StateSchema for MockState {
    fn merge(current: &Self, update: Self) -> Self {
        let mut new_state = current.clone();
        if !update.input.is_empty() {
            new_state.input = update.input;
        }
        // Append scratchpad steps
        new_state.scratchpad.extend(update.scratchpad);
        
        if update.final_output.is_some() {
            new_state.final_output = update.final_output;
        }
        
        // Take max iteration count? Or purely local?
        // Usually iteration count is kept in the loop context, 
        // but StateSchema can merge it if needed. 
        // For ReAct, we usually just want to track it.
        new_state.iteration_count = update.iteration_count.max(current.iteration_count);
        
        new_state
    }
}

impl HasUserInput for MockState {
    fn user_input(&self) -> &str {
        &self.input
    }
}

impl HasFinalOutput for MockState {
    fn final_output(&self) -> Option<&str> {
        self.final_output.as_deref()
    }
    
    fn set_final_output(&mut self, output: String) {
        self.final_output = Some(output);
    }
}

impl ScratchpadState for MockState {
    fn scratchpad(&self) -> &Vec<ReActStep> {
        &self.scratchpad
    }

    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> {
        &mut self.scratchpad
    }
    
    fn iteration_count(&self) -> u32 {
        self.iteration_count
    }

    fn increment_iteration(&mut self) {
        self.iteration_count += 1;
    }
}

// --- Mock Tool ---
struct MockTool {
    name: String,
    result: String,
}

#[async_trait::async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        "mock tool"
    }
    fn schema(&self) -> Value {
        Value::Null
    }
    async fn invoke(&self, _args: Value) -> Result<Value, ToolError> {
        Ok(Value::String(self.result.clone()))
    }
}

// --- Mock LLM ---
struct MockLlm {
    responses: Mutex<Vec<LlmResponse>>,
}

impl MockLlm {
    fn new(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses: Mutex::new(responses),
        }
    }
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockLlm {
    async fn invoke(&self, _input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Err(WesichainError::Custom("No more mock responses".into()));
        }
        Ok(responses.remove(0))
    }

    fn stream<'a>(
        &'a self,
        _input: LlmRequest,
    ) -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
        stream::empty().boxed()
    }
}

#[async_trait::async_trait]
impl ToolCallingLlm for MockLlm {}

#[tokio::test]
async fn test_react_subgraph_execution() {
    // Scenario: User says "Hello".
    // 1. Agent thinks "I should use tool". Outputs Action(call_tool).
    // 2. Tool executes. Outputs Observation("tool_result").
    // 3. Agent thinks "I have result". Outputs FinalAnswer("Done").

    let tool = Arc::new(MockTool {
        name: "test_tool".to_string(),
        result: "success".to_string(),
    });

    let responses = vec![
        // Response 1: Call tool
        LlmResponse {
            content: "Thinking...".to_string(),
            tool_calls: vec![wesichain_core::ToolCall {
                id: "call_1".to_string(),
                name: "test_tool".to_string(),
                args: Value::Null,
            }],
        },
        // Response 2: Final answer
        LlmResponse {
            content: "Done".to_string(),
            tool_calls: vec![],
        },
    ];

    let llm = Arc::new(MockLlm::new(responses));

    let graph = ReActGraphBuilder::new()
        .llm(llm)
        .tools(vec![tool])
        .build::<MockState>()
        .expect("Failed to build graph");

    let initial_state = MockState {
        input: "Hello".to_string(),
        ..Default::default()
    };
    
    let result = graph.invoke(GraphState::new(initial_state)).await.expect("Execution failed");

    // Verify trace
    let steps = &result.data.scratchpad;
    assert_eq!(steps.len(), 4); // Thought, Action, Observation, FinalAnswer
    
    match &steps[0] {
        ReActStep::Thought(text) => assert_eq!(text, "Thinking..."),
        _ => panic!("Expected Thought"),
    }
    match &steps[1] {
        ReActStep::Action(call) => assert_eq!(call.name, "test_tool"),
        _ => panic!("Expected Action"),
    }
    match &steps[2] {
        // Observation logic in decomposed graph might vary slightly in order depending on ReActToolNode?
        // ReActToolNode appends Observation.
        ReActStep::Observation(val) => assert_eq!(val.to_string(), "\"success\""), // Value::String debug format?
        _ => panic!("Expected Observation, got {:?} at index 2", steps[2]),
    }
    match &steps[3] {
        ReActStep::FinalAnswer(text) => assert_eq!(text, "Done"),
        _ => panic!("Expected FinalAnswer"),
    }
}
