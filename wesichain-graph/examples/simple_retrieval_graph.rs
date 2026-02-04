// Run: cargo run -p wesichain-graph --example simple_retrieval_graph

use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{GraphBuilder, GraphError, GraphState, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct RagState {
    query: String,
    docs: Vec<String>,
    answer: Option<String>,
}

impl StateSchema for RagState {
    fn merge(current: &Self, update: Self) -> Self {
        let query = if update.query.is_empty() {
            current.query.clone()
        } else {
            update.query
        };

        let mut docs = current.docs.clone();
        docs.extend(update.docs);

        let answer = if update.answer.is_some() {
            update.answer
        } else {
            current.answer.clone()
        };

        Self { query, docs, answer }
    }
}

struct Retriever;

#[async_trait]
impl Runnable<GraphState<RagState>, StateUpdate<RagState>> for Retriever {
    async fn invoke(
        &self,
        input: GraphState<RagState>,
    ) -> Result<StateUpdate<RagState>, WesichainError> {
        let docs = vec![
            format!("doc for '{}'", input.data.query),
            "doc: wesichain is rust-native".to_string(),
        ];
        Ok(StateUpdate::new(RagState {
            query: input.data.query,
            docs,
            answer: None,
        }))
    }

    fn stream(
        &self,
        _input: GraphState<RagState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::empty().boxed()
    }
}

struct Generator;

#[async_trait]
impl Runnable<GraphState<RagState>, StateUpdate<RagState>> for Generator {
    async fn invoke(
        &self,
        input: GraphState<RagState>,
    ) -> Result<StateUpdate<RagState>, WesichainError> {
        let answer = format!(
            "Answer based on {} docs: {}",
            input.data.docs.len(),
            input.data.docs.join(" | ")
        );
        Ok(StateUpdate::new(RagState {
            query: String::new(),
            docs: Vec::new(),
            answer: Some(answer),
        }))
    }

    fn stream(
        &self,
        _input: GraphState<RagState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::empty().boxed()
    }
}

#[tokio::main]
async fn main() -> Result<(), GraphError> {
    let graph = GraphBuilder::new()
        .add_node("retriever", Retriever)
        .add_node("llm", Generator)
        .add_edge("retriever", "llm")
        .set_entry("retriever")
        .build();

    let state = GraphState::new(RagState {
        query: "wesichain".to_string(),
        ..RagState::default()
    });
    let out = graph.invoke_graph(state).await?;
    println!("{}", out.data.answer.unwrap_or_default());
    Ok(())
}
