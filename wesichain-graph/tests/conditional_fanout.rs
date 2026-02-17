use wesichain_core::WesichainError;
use wesichain_graph::{
    ExecutionConfig, GraphBuilder, GraphContext, GraphNode, GraphState, StateSchema, StateUpdate,
    END,
};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct FanoutState {
    logs: Vec<String>,
}

impl StateSchema for FanoutState {
    fn merge(current: &Self, update: Self) -> Self {
        let mut logs = current.logs.clone();
        logs.extend(update.logs);
        FanoutState { logs }
    }
}

struct LoggerNode {
    id: String,
}

#[async_trait::async_trait]
impl GraphNode<FanoutState> for LoggerNode {
    async fn invoke_with_context(
        &self,
        _input: GraphState<FanoutState>,
        _context: &GraphContext,
    ) -> Result<StateUpdate<FanoutState>, WesichainError> {
        let update = FanoutState {
            logs: vec![self.id.clone()],
        };
        Ok(StateUpdate::new(update))
    }
}

#[tokio::test]
async fn test_conditional_fanout() {
    // Disable cycle detection to allow "diamond" pattern (A->B->D, A->C->D)
    // which simpler cycle detectors might flag as visiting D twice in same path history logic if not careful,
    // though strictly it's not a cycle. But our current simple detector might be aggressive.
    let config = ExecutionConfig {
        cycle_detection: false,
        ..Default::default()
    };

    let builder = GraphBuilder::<FanoutState>::new()
        .with_default_config(config)
        .add_node(
            "A",
            LoggerNode {
                id: "A".to_string(),
            },
        )
        .add_node(
            "B",
            LoggerNode {
                id: "B".to_string(),
            },
        )
        .add_node(
            "C",
            LoggerNode {
                id: "C".to_string(),
            },
        )
        .add_node(
            "D",
            LoggerNode {
                id: "D".to_string(),
            },
        )
        .set_entry("A")
        // Conditional edge from A returns both B and C
        .add_conditional_edge("A", |state: &GraphState<FanoutState>| {
            if state.data.logs.contains(&"A".to_string()) {
                vec!["B".to_string(), "C".to_string()]
            } else {
                vec![END.to_string()]
            }
        })
        .add_edge("B", "D")
        .add_edge("C", "D")
        .add_edge("D", END);

    let graph = builder.build();

    let initial_state = GraphState::new(FanoutState::default());
    let result = graph.invoke(initial_state).await.expect("Graph failed");

    let logs = result.data.logs;
    println!("Logs: {:?}", logs);

    // A runs first.
    assert_eq!(logs[0], "A");

    // B and C run next (order undefined).
    assert!(logs.contains(&"B".to_string()));
    assert!(logs.contains(&"C".to_string()));

    // D runs twice (merge from B and C)
    let d_count = logs.iter().filter(|&l| l == "D").count();
    assert_eq!(d_count, 2);

    assert_eq!(logs.len(), 5);
}
