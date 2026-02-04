// Run: cargo run -p wesichain-graph --example persistent_conversation

use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{Checkpointer, GraphBuilder, GraphError, GraphState, InMemoryCheckpointer, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct ConversationState {
    messages: Vec<String>,
    turn: u32,
}

impl StateSchema for ConversationState {
    fn merge(current: &Self, update: Self) -> Self {
        let mut messages = current.messages.clone();
        messages.extend(update.messages);
        let turn = if update.turn == 0 { current.turn } else { update.turn };
        Self { messages, turn }
    }
}

struct Reply;

#[async_trait]
impl Runnable<GraphState<ConversationState>, StateUpdate<ConversationState>> for Reply {
    async fn invoke(
        &self,
        input: GraphState<ConversationState>,
    ) -> Result<StateUpdate<ConversationState>, WesichainError> {
        let next_turn = input.data.turn + 1;
        let last = input
            .data
            .messages
            .last()
            .map(String::as_str)
            .unwrap_or("");
        let reply = format!("Turn {next_turn}: got '{last}'");
        Ok(StateUpdate::new(ConversationState {
            messages: vec![reply],
            turn: next_turn,
        }))
    }

    fn stream(
        &self,
        _input: GraphState<ConversationState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::empty().boxed()
    }
}

#[tokio::main]
async fn main() -> Result<(), GraphError> {
    let checkpointer = InMemoryCheckpointer::default();
    let graph = GraphBuilder::new()
        .add_node("reply", Reply)
        .set_entry("reply")
        .with_checkpointer(checkpointer.clone(), "thread-1")
        .build();

    let state = GraphState::new(ConversationState {
        messages: vec!["Hello".to_string()],
        turn: 0,
    });

    let out = graph.invoke_graph(state).await?;
    println!("After first turn: {}", out.data.messages.last().unwrap());

    let checkpoint = checkpointer
        .load("thread-1")
        .await?
        .expect("checkpoint");
    println!("Resuming from step {}", checkpoint.step);

    let resumed = graph.invoke_graph(checkpoint.state).await?;
    println!("After resume: {}", resumed.data.messages.last().unwrap());
    Ok(())
}
