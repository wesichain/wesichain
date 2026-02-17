use serde::{Deserialize, Serialize};
use wesichain_core::{HasFinalOutput, HasUserInput, ReActStep, ScratchpadState, WesichainError};
use wesichain_graph::{
    ExecutionOptions, GraphBuilder, GraphContext, GraphError, GraphNode, GraphState, StateSchema,
    StateUpdate, END,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TestState {
    value: Vec<String>,
    #[serde(skip)]
    scratchpad: Vec<ReActStep>,
}

impl StateSchema for TestState {
    fn merge(current: &Self, update: Self) -> Self {
        let mut new_val = current.value.clone();
        for v in update.value {
            if !new_val.contains(&v) {
                new_val.push(v);
            }
        }
        TestState {
            value: new_val,
            scratchpad: current.scratchpad.clone(),
        }
    }
}
impl HasUserInput for TestState {
    fn user_input(&self) -> &str {
        ""
    }
}
impl HasFinalOutput for TestState {
    fn final_output(&self) -> Option<&str> {
        None
    }
    fn set_final_output(&mut self, _: String) {}
}
impl ScratchpadState for TestState {
    fn scratchpad(&self) -> &Vec<ReActStep> {
        &self.scratchpad
    }
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> {
        &mut self.scratchpad
    }
    fn iteration_count(&self) -> u32 {
        0
    }
    fn increment_iteration(&mut self) {}
}

struct PassNode {
    name: String,
}

#[async_trait::async_trait]
impl GraphNode<TestState> for PassNode {
    async fn invoke_with_context(
        &self,
        _: GraphState<TestState>,
        _: &GraphContext,
    ) -> Result<StateUpdate<TestState>, WesichainError> {
        Ok(StateUpdate::new(TestState {
            value: vec![self.name.clone()],
            ..Default::default()
        }))
    }
}

// Scenario 1: Diamond Pattern (Valid DAG)
// A -> B -> D
// A -> C -> D
// D should be visited twice, but via different paths.
// With Path-Sensitive detection, this should NOT exceed loop limits even if limit is 1 (per path).
// (Actually limit must be >= 1 to visit once).
#[tokio::test]
async fn test_diamond_pattern() {
    let builder = GraphBuilder::<TestState>::new()
        .add_node(
            "A",
            PassNode {
                name: "A".to_string(),
            },
        )
        .add_node(
            "B",
            PassNode {
                name: "B".to_string(),
            },
        )
        .add_node(
            "C",
            PassNode {
                name: "C".to_string(),
            },
        )
        .add_node(
            "D",
            PassNode {
                name: "D".to_string(),
            },
        )
        .set_entry("A")
        .add_edge("A", "B")
        .add_edge("A", "C")
        .add_edge("B", "D")
        .add_edge("C", "D")
        .add_edge("D", END);

    let graph = builder.build();
    let input = GraphState::new(TestState::default());

    // Strict limits: max_visits=2 (global), max_loop=1 (per path)
    // D is visited twice globally (OK <= 2).
    // D is visited once per path (OK <= 1).
    let options = ExecutionOptions {
        max_visits: Some(10), // Allow global visits
        max_loop_iterations: Some(1),
        cycle_detection: Some(false), // Disable standard check to allow diamond pattern
        ..Default::default()
    };

    let result = graph.invoke_graph_with_options(input, options).await;
    assert!(
        result.is_ok(),
        "Diamond pattern should succeed: with max_loop=1, D visited once per path"
    );

    let state = result.unwrap();
    // A, B, C, D should be in value
    assert!(state.data.value.contains(&"D".to_string()));
}

// Scenario 2: Simple Loop
// A -> B -> A
// Should fail after max_loop_iterations.
#[tokio::test]
async fn test_infinite_loop() {
    let builder = GraphBuilder::<TestState>::new()
        .add_node(
            "A",
            PassNode {
                name: "A".to_string(),
            },
        )
        .add_node(
            "B",
            PassNode {
                name: "B".to_string(),
            },
        )
        .set_entry("A")
        .add_edge("A", "B")
        .add_edge("B", "A");

    let graph = builder.build();
    let input = GraphState::new(TestState::default());

    // max_loop_iterations = 3.
    // A(1) -> B(1) -> A(2) -> B(2) -> A(3) -> B(3) -> A(4) FAIL.
    // Path ID should be preserved so count accumulates.
    let options = ExecutionOptions {
        max_visits: Some(100), // High global limit
        max_loop_iterations: Some(3),
        cycle_detection: Some(false), // Disable standard cycle detection to test our new logic
        max_steps: Some(100),
        ..Default::default()
    };

    let result = graph.invoke_graph_with_options(input, options).await;

    match result {
        Err(GraphError::MaxLoopIterationsExceeded { node, max, .. }) => {
            assert_eq!(node, "A");
            assert_eq!(max, 3);
        }
        _ => panic!("Expected MaxLoopIterationsExceeded, got {:?}", result),
    }
}
