use wesichain_core::WesichainError;
use wesichain_graph::{
    ExecutionConfig, GraphBuilder, GraphNode, GraphContext, GraphState, StateSchema, StateUpdate, END,
};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct ConcurrencyState {
    logs: Vec<String>,
}

impl StateSchema for ConcurrencyState {
    fn merge(current: &Self, update: Self) -> Self {
        let mut logs = current.logs.clone();
        logs.extend(update.logs);
        ConcurrencyState { logs }
    }
}

struct LoggerNode {
    id: String,
}

#[async_trait::async_trait]
impl GraphNode<ConcurrencyState> for LoggerNode {
    async fn invoke_with_context(
        &self,
        _input: GraphState<ConcurrencyState>,
        _context: &GraphContext,
    ) -> Result<StateUpdate<ConcurrencyState>, WesichainError> {
        let update = ConcurrencyState {
            logs: vec![self.id.clone()],
        };
        Ok(StateUpdate::new(update))
    }
}

#[tokio::test]
async fn test_static_fanout() {
    let config = ExecutionConfig {
        cycle_detection: false,
        ..Default::default()
    };

    let builder = GraphBuilder::<ConcurrencyState>::new()
        .with_default_config(config)
        .add_node("A", LoggerNode { id: "A".to_string() })
        .add_node("B", LoggerNode { id: "B".to_string() })
        .add_node("C", LoggerNode { id: "C".to_string() })
        .add_node("D", LoggerNode { id: "D".to_string() })
        .set_entry("A")
        .add_edges("A", &["B", "C"])
        .add_edge("B", "D")
        .add_edge("C", "D")
        .add_edge("D", END);

    let graph = builder.build();
    
    let initial_state = GraphState::new(ConcurrencyState::default());
    let result = graph.invoke(initial_state).await.expect("Graph failed");
    
    let logs = result.data.logs;
    println!("Logs: {:?}", logs);
    
    // A runs first.
    assert_eq!(logs[0], "A");
    
    // B and C run next (order undefined in queue, but sequential).
    // Both B and C should be present.
    assert!(logs.contains(&"B".to_string()));
    assert!(logs.contains(&"C".to_string()));
    
    // D runs twice (once from B, once from C)
    let d_count = logs.iter().filter(|&l| l == "D").count();
    assert_eq!(d_count, 2);
    
    // Total count: 1 (A) + 1 (B) + 1 (C) + 2 (D) = 5
    assert_eq!(logs.len(), 5);
}
