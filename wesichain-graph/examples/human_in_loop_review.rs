// Run: cargo run -p wesichain-graph --example human_in_loop_review

use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{Checkpointer, GraphBuilder, GraphError, GraphState, InMemoryCheckpointer, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct ReviewState {
    value: i32,
    approved: bool,
}

impl StateSchema for ReviewState {}

struct Prepare;

#[async_trait]
impl Runnable<GraphState<ReviewState>, StateUpdate<ReviewState>> for Prepare {
    async fn invoke(
        &self,
        input: GraphState<ReviewState>,
    ) -> Result<StateUpdate<ReviewState>, WesichainError> {
        Ok(StateUpdate::new(ReviewState {
            value: input.data.value + 1,
            approved: false,
        }))
    }

    fn stream(
        &self,
        _input: GraphState<ReviewState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::empty().boxed()
    }
}

struct Review;

#[async_trait]
impl Runnable<GraphState<ReviewState>, StateUpdate<ReviewState>> for Review {
    async fn invoke(
        &self,
        input: GraphState<ReviewState>,
    ) -> Result<StateUpdate<ReviewState>, WesichainError> {
        Ok(StateUpdate::new(ReviewState {
            value: input.data.value,
            approved: true,
        }))
    }

    fn stream(
        &self,
        _input: GraphState<ReviewState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::empty().boxed()
    }
}

#[tokio::main]
async fn main() -> Result<(), GraphError> {
    let checkpointer = InMemoryCheckpointer::default();
    let graph = GraphBuilder::new()
        .add_node("prepare", Prepare)
        .add_node("review", Review)
        .add_edge("prepare", "review")
        .set_entry("prepare")
        .with_checkpointer(checkpointer.clone(), "thread-42")
        .with_interrupt_before(["review"])
        .build();

    let state = GraphState::new(ReviewState { value: 0, approved: false });
    match graph.invoke_graph(state).await {
        Err(GraphError::Interrupted) => {
            let checkpoint = checkpointer
                .load("thread-42")
                .await?
                .expect("checkpoint");
            println!("Paused at step {} (node: {})", checkpoint.step, checkpoint.node);

            let resume_graph = GraphBuilder::new()
                .add_node("prepare", Prepare)
                .add_node("review", Review)
                .add_edge("prepare", "review")
                .set_entry("prepare")
                .build();

            let out = resume_graph.invoke_graph(checkpoint.state).await?;
            println!("Approved: {}", out.data.approved);
        }
        Ok(out) => println!("Completed without interrupt: {}", out.data.approved),
        Err(err) => return Err(err),
    }

    Ok(())
}
