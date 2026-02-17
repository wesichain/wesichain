use std::time::Duration;
use tokio::time::sleep;

use wesichain_core::{
    HasFinalOutput, HasUserInput, ScratchpadState, ReActStep, WesichainError,
};
use wesichain_graph::{
    GraphContext, GraphState, GraphBuilder, GraphNode, StateSchema, StateUpdate, END,
    GraphError, ExecutionOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TestState {
    value: Vec<String>,
    #[serde(skip)]
    scratchpad: Vec<ReActStep>,
}

impl StateSchema for TestState {
    fn merge(current: &Self, update: Self) -> Self {
        let mut new_val = current.value.clone();
        new_val.extend(update.value);
        TestState {
            value: new_val,
            scratchpad: current.scratchpad.clone(),
        }
    }
}
impl HasUserInput for TestState { fn user_input(&self) -> &str { "" } }
impl HasFinalOutput for TestState { fn final_output(&self) -> Option<&str> { None } fn set_final_output(&mut self, _: String) {} }
impl ScratchpadState for TestState { 
    fn scratchpad(&self) -> &Vec<ReActStep> { &self.scratchpad } 
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> { &mut self.scratchpad } 
    fn iteration_count(&self) -> u32 { 0 } 
    fn increment_iteration(&mut self) {} 
}

struct SleepNode {
    name: String,
    delay: u64,
}

#[async_trait::async_trait]
impl GraphNode<TestState> for SleepNode {
    async fn invoke_with_context(&self, _: GraphState<TestState>, _: &GraphContext) -> Result<StateUpdate<TestState>, WesichainError> {
        sleep(Duration::from_millis(self.delay)).await;
        Ok(StateUpdate::new(TestState {
            value: vec![self.name.clone()],
            ..Default::default()
        }))
    }
}

#[tokio::test]
async fn test_global_timeout() {
    // Sequential chain: A -> B -> C (3 x 50ms = 150ms total)
    // vs 70ms timeout
    let builder = GraphBuilder::<TestState>::new()
        .add_node("A", SleepNode { name: "A".to_string(), delay: 50 })
        .add_node("B", SleepNode { name: "B".to_string(), delay: 50 })
        .add_node("C", SleepNode { name: "C".to_string(), delay: 50 })
        .set_entry("A")
        .add_edge("A", "B")
        .add_edge("B", "C")
        .add_edge("C", END);

    let graph = builder.build();
    let input = GraphState::new(TestState::default());
    
    // Set max duration to 70ms, total execution ~150ms
    let options = ExecutionOptions {
        max_duration: Some(Duration::from_millis(70)),
        ..Default::default()
    };

    let result = graph.invoke_graph_with_options(input, options).await;
    
    match result {
        Err(GraphError::Timeout { .. }) => assert!(true),
        _ => panic!("Expected Timeout error, got {:?}", result),
    }
}

#[tokio::test]
async fn test_node_timeout() {
    let builder = GraphBuilder::<TestState>::new()
        .add_node("slow_node", SleepNode { name: "A".to_string(), delay: 200 })
        .set_entry("slow_node")
        .add_edge("slow_node", END);

    let graph = builder.build();
    let input = GraphState::new(TestState::default());
    
    // Set node timeout to 50ms, task takes 200ms
    let options = ExecutionOptions {
        node_timeout: Some(Duration::from_millis(50)),
        ..Default::default()
    };

    let result = graph.invoke_graph_with_options(input, options).await;
    
    // Node timeout is wrapped in NodeFailed in main loop (spawn result returns Err)
    // Or if we implemented logic to return specific NodeTimeout error?
    // In graph.rs: 
    // Err(_) => Err(WesichainError::Custom(format!("Node {} timed out...", node_id))),
    // And join_set result logic:
    // match result { Ok(update) => ..., Err(err) => ... }
    // If Result<StateUpdate, WesichainError> is Err, it wraps in GraphError::NodeFailed.
    
    match result {
        Err(GraphError::NodeFailed { source, .. }) => {
            let msg = source.to_string();
            assert!(msg.contains("timed out"));
        },
        _ => panic!("Expected NodeFailed with timeout, got {:?}", result),
    }
}

#[tokio::test]
async fn test_max_visits() {
    // Ping pong: A -> B -> A ...
    let builder = GraphBuilder::<TestState>::new()
        .add_node("A", SleepNode { name: "A".to_string(), delay: 10 })
        .add_node("B", SleepNode { name: "B".to_string(), delay: 10 })
        .set_entry("A")
        .add_edge("A", "B")
        .add_edge("B", "A");

    let graph = builder.build();
    let input = GraphState::new(TestState::default());
    
    // Max visits 3. 
    // Steps: 
    // 1. A (count=1)
    // 2. B (count=1) 
    // 3. A (count=2)
    // 4. B (count=2)
    // 5. A (count=3)
    // 6. B (count=3)
    // 7. A (count=4) -> FAIL
    let options = ExecutionOptions {
        max_visits: Some(3),
        max_steps: Some(100), // High enough to not hit first
        cycle_detection: Some(false), // Disable to test max_visits specifically
        ..Default::default()
    };

    let result = graph.invoke_graph_with_options(input, options).await;
    
    match result {
        Err(GraphError::MaxVisitsExceeded { node, max }) => {
            assert_eq!(node, "A");
            assert_eq!(max, 3);
        },
        _ => panic!("Expected MaxVisitsExceeded error, got {:?}", result),
    }
}
