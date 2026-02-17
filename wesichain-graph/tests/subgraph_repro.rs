use std::sync::Arc;
use futures::StreamExt;
use wesichain_core::{Runnable, WesichainError};
use wesichain_graph::{GraphBuilder, GraphState, StateSchema, StateUpdate, START, END};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct TestState {
    value: String,
}

impl StateSchema for TestState {
    fn merge(current: &Self, update: Self) -> Self {
        TestState {
            value: format!("{} -> {}", current.value, update.value),
        }
    }
}

struct AppendNode {
    suffix: String,
}

#[async_trait::async_trait]
impl Runnable<GraphState<TestState>, StateUpdate<TestState>> for AppendNode {
    async fn invoke(&self, input: GraphState<TestState>) -> Result<StateUpdate<TestState>, WesichainError> {
        Ok(StateUpdate::new(TestState {
            value: self.suffix.clone(),
        }))
    }

    fn stream<'a>(
        &'a self,
        _input: GraphState<TestState>,
    ) -> futures::stream::BoxStream<'a, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
async fn test_subgraph_composition() {
    // 1. Build Subgraph
    let subgraph = GraphBuilder::<TestState>::new()
        .add_node("sub_node", AppendNode { suffix: "sub".to_string() })
        .add_edge(START, "sub_node")
        .add_edge("sub_node", END)
        .set_entry("sub_node")
        .build();

    // 2. Build Parent Graph wrapping Subgraph
    // This fails to compile if ExecutableGraph doesn't implement Runnable (or GraphNode)
    let parent = GraphBuilder::<TestState>::new()
        .add_node("subgraph_node", subgraph) 
        .add_edge(START, "subgraph_node")
        .add_edge("subgraph_node", END)
        .set_entry("subgraph_node")
        .build();

    // 3. Execute
    let input = GraphState::new(TestState { value: "start".to_string() });
    let result = parent.invoke(input).await.expect("Execution failed");

    // Expected: start -> sub
    // Expected: start (initial) merged with subgraph result (start -> sub)
    // resulting in: start -> start -> sub
    assert_eq!(result.data.value, "start -> start -> sub");
}
