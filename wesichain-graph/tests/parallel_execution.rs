use std::time::Duration;
use tokio::time::sleep;
use wesichain_core::WesichainError;
use wesichain_graph::{
    ExecutionConfig, GraphBuilder, GraphContext, GraphNode, GraphState, StateSchema, StateUpdate,
    END,
};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct ParallelState {
    logs: Vec<String>,
}

impl StateSchema for ParallelState {
    type Update = Self;

    fn apply(current: &Self, update: Self) -> Self {
        let mut logs = current.logs.clone();
        logs.extend(update.logs);
        ParallelState { logs }
    }
}

struct SleepyNode {
    id: String,
    delay_ms: u64,
}

#[async_trait::async_trait]
impl GraphNode<ParallelState> for SleepyNode {
    async fn invoke_with_context(
        &self,
        _input: GraphState<ParallelState>,
        _context: &GraphContext,
    ) -> Result<StateUpdate<ParallelState>, WesichainError> {
        sleep(Duration::from_millis(self.delay_ms)).await;
        let update = ParallelState {
            logs: vec![self.id.clone()],
        };
        Ok(StateUpdate::new(update))
    }
}

#[tokio::test]
async fn test_parallel_execution() {
    let config = ExecutionConfig {
        cycle_detection: false, // Not needed for simple fanout
        ..Default::default()
    };

    // Graph: A -> [B (200ms), C (200ms)] -> D
    // Total time should be ~200ms (+ overhead), definitely < 400ms.
    let builder = GraphBuilder::<ParallelState>::new()
        .with_default_config(config)
        .add_node(
            "A",
            SleepyNode {
                id: "A".to_string(),
                delay_ms: 10,
            },
        )
        .add_node(
            "B",
            SleepyNode {
                id: "B".to_string(),
                delay_ms: 200,
            },
        )
        .add_node(
            "C",
            SleepyNode {
                id: "C".to_string(),
                delay_ms: 200,
            },
        )
        .add_node(
            "D",
            SleepyNode {
                id: "D".to_string(),
                delay_ms: 10,
            },
        )
        .set_entry("A")
        .add_conditional_edge("A", |_| vec!["B".to_string(), "C".to_string()])
        .add_edge("B", "D")
        .add_edge("C", "D")
        .add_edge("D", END);

    let graph = builder.build();

    let initial_state = GraphState::new(ParallelState::default());

    let start = std::time::Instant::now();
    let result = graph.invoke(initial_state).await.expect("Graph failed");
    let duration = start.elapsed();

    println!("Total duration: {:?}", duration);
    println!("Logs: {:?}", result.data.logs);

    // Verify execution
    let logs = result.data.logs;
    assert!(logs.contains(&"A".to_string()));
    assert!(logs.contains(&"B".to_string()));
    assert!(logs.contains(&"C".to_string()));
    assert!(logs.contains(&"D".to_string()));

    // Check timing
    // With parallel execution: 10ms (A) + max(200, 200) + 10ms (D) = ~220ms.
    // Sequential would be ~420ms.
    // Allow generous margin for overhead but check < 350ms.
    assert!(
        duration < Duration::from_millis(350),
        "Execution took {:?}, expected < 350ms (parallel)",
        duration
    );
    assert!(duration > Duration::from_millis(200), "Execution too fast!");
}
