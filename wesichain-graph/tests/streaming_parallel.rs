use std::time::Duration;
use tokio::time::sleep;
use futures::StreamExt;

use wesichain_core::{
    HasFinalOutput, HasUserInput, ScratchpadState, ReActStep, WesichainError,
};
use wesichain_graph::{
    GraphContext, GraphState, GraphBuilder, GraphNode, StateSchema, StateUpdate, END,
    GraphEvent,
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
            scratchpad: current.scratchpad.clone(), // Keep scratchpad
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
async fn test_streaming_parallel() {
    let builder = GraphBuilder::<TestState>::new()
        .add_node("A", SleepNode { name: "A".to_string(), delay: 200 })
        .add_node("B", SleepNode { name: "B".to_string(), delay: 200 })
        .set_entry("start_node")
        // Start -> [A, B] via conditional fan-out
        .add_conditional_edge("start_node", |_| vec!["A".to_string(), "B".to_string()])
        .add_edge("A", END)
        .add_edge("B", END);
        
    // We need a start node that does nothing but return empty update to trigger fan-out
    struct StartNode;
    #[async_trait::async_trait]
    impl GraphNode<TestState> for StartNode {
        async fn invoke_with_context(&self, _: GraphState<TestState>, _: &GraphContext) -> Result<StateUpdate<TestState>, WesichainError> {
             Ok(StateUpdate::new(TestState::default()))
        }
    }
    
    let builder = builder.add_node("start_node", StartNode);

    let graph = builder.build();
    let input = GraphState::new(TestState::default());
    
    let mut stream = graph.stream_invoke(input);
    let mut events = Vec::new();
    let start = std::time::Instant::now();
    
    while let Some(evt) = stream.next().await {
        let evt = evt.expect("Stream error");
        println!("{:?}", evt);
        events.push(evt);
    }
    
    let duration = start.elapsed();
    println!("Total duration: {:?}", duration);
    
    // Duration should be ~200ms (parallel), not 400ms.
    assert!(duration < Duration::from_millis(350));
    
    // Check events
    // Expect: NodeEnter(start) -> NodeExit(start) -> NodeEnter(A) & NodeEnter(B) (order may vary or close) -> NodeFinished(A/B) -> ...
    
    let active_nodes: Vec<_> = events.iter().filter_map(|e| match e {
        GraphEvent::NodeEnter { node, .. } => Some(node.clone()),
        _ => None
    }).collect();
    
    assert!(active_nodes.contains(&"A".to_string()));
    assert!(active_nodes.contains(&"B".to_string()));
    
    // Check timestamps exist
    match &events[0] {
        GraphEvent::NodeEnter { timestamp, .. } => assert!(*timestamp > 0),
        _ => {}
    }
}
