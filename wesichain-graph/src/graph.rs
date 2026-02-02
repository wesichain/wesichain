use std::collections::HashMap;

use crate::{Checkpoint, Checkpointer, GraphState, StateSchema, StateUpdate};
use wesichain_core::{Runnable, WesichainError};

pub type Condition<S> = Box<dyn Fn(&GraphState<S>) -> String + Send + Sync>;

pub struct GraphBuilder<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    edges: HashMap<String, String>,
    conditional: HashMap<String, Condition<S>>,
    checkpointer: Option<(Box<dyn Checkpointer<S>>, String)>,
    entry: Option<String>,
}

impl<S: StateSchema> Default for GraphBuilder<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: StateSchema> GraphBuilder<S> {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            conditional: HashMap::new(),
            checkpointer: None,
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

    pub fn add_conditional_edge<F>(mut self, from: &str, condition: F) -> Self
    where
        F: Fn(&GraphState<S>) -> String + Send + Sync + 'static,
    {
        self.conditional
            .insert(from.to_string(), Box::new(condition));
        self
    }

    pub fn with_checkpointer<C>(mut self, checkpointer: C, thread_id: &str) -> Self
    where
        C: Checkpointer<S> + 'static,
    {
        self.checkpointer = Some((Box::new(checkpointer), thread_id.to_string()));
        self
    }

    pub fn build(self) -> ExecutableGraph<S> {
        ExecutableGraph {
            nodes: self.nodes,
            edges: self.edges,
            conditional: self.conditional,
            checkpointer: self.checkpointer,
            entry: self.entry.expect("entry"),
        }
    }
}

pub struct ExecutableGraph<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    edges: HashMap<String, String>,
    conditional: HashMap<String, Condition<S>>,
    checkpointer: Option<(Box<dyn Checkpointer<S>>, String)>,
    entry: String,
}

impl<S: StateSchema> ExecutableGraph<S> {
    pub async fn invoke(&self, mut state: GraphState<S>) -> Result<GraphState<S>, WesichainError> {
        let mut current = self.entry.clone();
        loop {
            let node = self.nodes.get(&current).expect("node");
            let update = node.invoke(state).await?;
            state = GraphState::new(update.data);
            if let Some((checkpointer, thread_id)) = &self.checkpointer {
                let checkpoint = Checkpoint::new(thread_id.clone(), state.clone());
                checkpointer
                    .save(&checkpoint)
                    .await
                    .map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?;
            }
            if let Some(condition) = self.conditional.get(&current) {
                current = condition(&state);
                continue;
            }
            match self.edges.get(&current) {
                Some(next) => current = next.clone(),
                None => break,
            }
        }
        Ok(state)
    }
}
