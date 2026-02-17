use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use wesichain_core::{
    HasFinalOutput, HasUserInput, ScratchpadState, Message, ReActStep, Tool, ToolCall, Value, WesichainError, Runnable
};
// ToolError is likely in wesichain_core or wesichain_agent. 
// Since I cannot ensure where it is, I will try to look at compilation error again or use Box<dyn Error>.
// Actually, I should use `wesichain_core::ToolError` if available.
// Let's assume it is available as `wesichain_core::ToolError`.
// But wait, the previous error said `found WesichainError` in my impl.
// Let's guess it is `wesichain_core::error::ToolError` or just `wesichain_core::ToolError`.
// If I can't find it, I will use `anyhow::Result` if trait allows, but trait is strict.
// I'll try to find it via grep or just implement `invoke` to return what trait wants using `Box`.
// Error said: `expected ToolError`.
// I will attempt to alias it if generic, or find where it is imported.
// `wesichain_agent` defines `Tool`? `wesichain_core` defines `Tool`?
// `wesichain-graph` imports `wesichain_core::{..., Tool, ...}`.
// Let's assume `ToolError` is also in `wesichain_core`.

use wesichain_graph::{
    GraphContext, GraphState, StateSchema, StateUpdate, GraphNode,
    react_subgraph::{ReActToolNode, ToolFailurePolicy},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TestState {
    input: String,
    output: Option<String>,
    scratchpad: Vec<ReActStep>,
    iteration: u32,
}

impl StateSchema for TestState {}
impl HasUserInput for TestState {
    fn user_input(&self) -> &str {
        &self.input
    }
}
impl HasFinalOutput for TestState {
    fn final_output(&self) -> Option<&str> {
        self.output.as_deref()
    }
    fn set_final_output(&mut self, output: String) {
        self.output = Some(output);
    }
}
impl ScratchpadState for TestState {
    fn scratchpad(&self) -> &Vec<ReActStep> {
        &self.scratchpad
    }
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> {
        &mut self.scratchpad
    }
    fn iteration_count(&self) -> u32 {
        self.iteration
    }
    fn increment_iteration(&mut self) {
        self.iteration += 1;
    }
}

struct SlowTool {
    delay_ms: u64,
}

#[async_trait::async_trait]
impl Tool for SlowTool {
    fn name(&self) -> &str {
        "slow_tool"
    }
    fn description(&self) -> &str {
        "Sleeps for a while"
    }
    fn schema(&self) -> Value {
        Value::Null
    }
    // Correct signature matching trait
    async fn invoke(&self, args: Value) -> Result<Value, wesichain_core::ToolError> {
        sleep(Duration::from_millis(self.delay_ms)).await;
        // Return arg as string
        Ok(Value::String(format!("Slept {}ms with {:?}", self.delay_ms, args)))
    }
}

#[tokio::test]
async fn test_parallel_tool_node() {
    let tool = Arc::new(SlowTool { delay_ms: 200 });
    let mut tools_map: HashMap<String, Arc<dyn Tool>> = HashMap::new();
    tools_map.insert(tool.name().to_string(), tool);

    let node = ReActToolNode::new(tools_map, ToolFailurePolicy::FailFast);

    // Create state with 3 concurrent tool calls
    let mut state = TestState::default();
    state.scratchpad.push(ReActStep::Action(ToolCall {
        name: "slow_tool".to_string(),
        args: Value::String("call1".to_string()),
        id: "id1".to_string(),
    }));
    state.scratchpad.push(ReActStep::Action(ToolCall {
        name: "slow_tool".to_string(),
        args: Value::String("call2".to_string()),
        id: "id2".to_string(),
    }));
    state.scratchpad.push(ReActStep::Action(ToolCall {
        name: "slow_tool".to_string(),
        args: Value::String("call3".to_string()),
        id: "id3".to_string(),
    }));

    let input = GraphState::new(state);
    let context = GraphContext {
        remaining_steps: None,
        observer: None,
        node_id: "tools".to_string(),
    };

    let start = std::time::Instant::now();
    let result = node.invoke_with_context(input, &context).await.expect("Tool execution failed");
    let duration = start.elapsed();

    println!("Total tool execution time: {:?}", duration);

    // Verify duration: should be ~200ms, definitely < 600ms
    assert!(duration < Duration::from_millis(400), "Tools ran sequentially! Took {:?}", duration);
    assert!(duration > Duration::from_millis(150));

    // Verify ordering
    let steps = result.data.scratchpad;
    assert_eq!(steps.len(), 3);
    
    // Original actions: id1, id2, id3
    // Observations should correspond to these.
    // ReActToolNode logic: 
    // It pushes observations.
    // The outputs should match the inputs.
    // "Slept 200ms with String(\"call1\")"
    
    match &steps[0] {
        ReActStep::Observation(val) => assert!(val.to_string().contains("call1")),
        _ => panic!("Expected observation 1"),
    }
    match &steps[1] {
        ReActStep::Observation(val) => assert!(val.to_string().contains("call2")),
        _ => panic!("Expected observation 2"),
    }
    match &steps[2] {
        ReActStep::Observation(val) => assert!(val.to_string().contains("call3")),
        _ => panic!("Expected observation 3"),
    }
}
