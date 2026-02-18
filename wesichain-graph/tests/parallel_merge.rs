use std::time::Duration;
use tokio::time::sleep;
use wesichain_core::WesichainError;
use wesichain_graph::{
    state::{Append, Overwrite, Reducer},
    ExecutionConfig, GraphBuilder, GraphContext, GraphNode, GraphState, StateSchema, StateUpdate,
    END,
};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct MergeState {
    // Field 1: Uses Append reducer (simulated via manual merge impl for now)
    messages: Vec<String>,
    // Field 2: Uses Overwrite reducer (simulated)
    last_node: String,
}

impl StateSchema for MergeState {
    type Update = Self;

    fn apply(current: &Self, update: Self) -> Self {
        // Here we demonstrate how a user would use the Reducers
        let messages = Append.reduce(current.messages.clone(), update.messages);
        let last_node = Overwrite.reduce(current.last_node.clone(), update.last_node);

        MergeState {
            messages,
            last_node,
        }
    }
}

struct AppendingNode {
    id: String,
    delay_ms: u64,
}

#[async_trait::async_trait]
impl GraphNode<MergeState> for AppendingNode {
    async fn invoke_with_context(
        &self,
        _input: GraphState<MergeState>,
        _context: &GraphContext,
    ) -> Result<StateUpdate<MergeState>, WesichainError> {
        sleep(Duration::from_millis(self.delay_ms)).await;
        let update = MergeState {
            messages: vec![format!("msg from {}", self.id)],
            last_node: self.id.clone(),
        };
        Ok(StateUpdate::new(update))
    }
}

#[tokio::test]
async fn test_parallel_merge() {
    let config = ExecutionConfig {
        cycle_detection: false,
        ..Default::default()
    };

    // Graph: A -> [B (50ms), C (50ms)] -> END
    // B and C run in parallel. Both append to messages.
    // One will overwrite last_node (race condition for last_node, but deterministic set for messages).
    let builder = GraphBuilder::<MergeState>::new()
        .with_default_config(config)
        .add_node(
            "A",
            AppendingNode {
                id: "A".to_string(),
                delay_ms: 10,
            },
        )
        .add_node(
            "B",
            AppendingNode {
                id: "B".to_string(),
                delay_ms: 50,
            },
        )
        .add_node(
            "C",
            AppendingNode {
                id: "C".to_string(),
                delay_ms: 50,
            },
        )
        .set_entry("A")
        .add_conditional_edge("A", |_| vec!["B".to_string(), "C".to_string()])
        .add_edge("B", END)
        .add_edge("C", END);

    let graph = builder.build();

    let initial_state = GraphState::new(MergeState::default());
    let result = graph.invoke(initial_state).await.expect("Graph failed");

    let messages = result.data.messages;
    println!("Final messages: {:?}", messages);

    // A runs first.
    assert!(messages.contains(&"msg from A".to_string()));

    // B and C run parallel. Both should be in the list.
    assert!(messages.contains(&"msg from B".to_string()));
    assert!(messages.contains(&"msg from C".to_string()));

    // Total 3 messages.
    assert_eq!(messages.len(), 3);

    // last_node will be either B or C, depending on who finished last.
    println!("Final last_node: {}", result.data.last_node);
    assert!(result.data.last_node == "B" || result.data.last_node == "C");
}
