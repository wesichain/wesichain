use std::collections::HashMap;

use crate::{GraphState, StateSchema, StateUpdate};
use wesichain_core::{Runnable, WesichainError};

pub struct GraphBuilder<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    edges: HashMap<String, String>,
    entry: Option<String>,
}

impl<S: StateSchema> GraphBuilder<S> {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            entry: None,
        }
    }

    pub fn add_node<R>(mut self, name: &str, node: R) -> Self
    where
        R: Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync + 'static,
    {
        self.nodes.insert(name.to_string(), Box::new(node));
        self
    }

    pub fn set_entry(mut self, name: &str) -> Self {
        self.entry = Some(name.to_string());
        self
    }

    pub fn add_edge(mut self, from: &str, to: &str) -> Self {
        self.edges.insert(from.to_string(), to.to_string());
        self
    }

    pub fn build(self) -> ExecutableGraph<S> {
        ExecutableGraph {
            nodes: self.nodes,
            edges: self.edges,
            entry: self.entry.expect("entry"),
        }
    }
}

pub struct ExecutableGraph<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    edges: HashMap<String, String>,
    entry: String,
}

impl<S: StateSchema> ExecutableGraph<S> {
    pub async fn invoke(
        &self,
        mut state: GraphState<S>,
    ) -> Result<GraphState<S>, WesichainError> {
        let mut current = self.entry.clone();
        loop {
            let node = self.nodes.get(&current).expect("node");
            let update = node.invoke(state).await?;
            state = GraphState::new(update.data);
            match self.edges.get(&current) {
                Some(next) => current = next.clone(),
                None => break,
            }
        }
        Ok(state)
    }
}
