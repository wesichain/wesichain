use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{Checkpointer, GraphBuilder, GraphState, StateSchema, StateUpdate};

use wesichain_checkpoint_sqlite::SqliteCheckpointer;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

struct AddOne;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for AddOne {
    async fn invoke(
        &self,
        input: GraphState<DemoState>,
    ) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState {
            count: input.data.count + 1,
        }))
    }

    fn stream(
        &self,
        _input: GraphState<DemoState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
async fn checkpointer_sqlite_graph_builder_saves_checkpoint() {
    let checkpointer = SqliteCheckpointer::builder("sqlite::memory:")
        .max_connections(1)
        .build()
        .await
        .expect("sqlite checkpointer should build");
    let inspect = checkpointer.clone();

    let graph = GraphBuilder::new()
        .add_node("inc", AddOne)
        .set_entry("inc")
        .with_checkpointer(checkpointer, "thread-sqlite")
        .build();

    let output = graph
        .invoke(GraphState::new(DemoState { count: 1 }))
        .await
        .expect("graph invocation should succeed");

    assert_eq!(output.data.count, 2);

    let loaded: wesichain_graph::Checkpoint<DemoState> = inspect
        .load("thread-sqlite")
        .await
        .expect("checkpoint load should succeed")
        .expect("checkpoint should exist");

    assert_eq!(loaded.state.data.count, 2);
    assert_eq!(loaded.node, "inc");
}
