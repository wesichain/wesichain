use std::collections::{HashMap, VecDeque};

use crate::{
    Checkpoint, Checkpointer, ExecutionConfig, ExecutionOptions, GraphError, GraphState,
    StateSchema, StateUpdate,
};
use wesichain_core::{Runnable, WesichainError};

pub type Condition<S> = Box<dyn Fn(&GraphState<S>) -> String + Send + Sync>;

pub struct GraphBuilder<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    edges: HashMap<String, String>,
    conditional: HashMap<String, Condition<S>>,
    checkpointer: Option<(Box<dyn Checkpointer<S>>, String)>,
    default_config: ExecutionConfig,
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
            default_config: ExecutionConfig::default(),
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

    pub fn with_default_config(mut self, config: ExecutionConfig) -> Self {
        self.default_config = config;
        self
    }

    pub fn build(self) -> ExecutableGraph<S> {
        ExecutableGraph {
            nodes: self.nodes,
            edges: self.edges,
            conditional: self.conditional,
            checkpointer: self.checkpointer,
            default_config: self.default_config,
            entry: self.entry.expect("entry"),
        }
    }
}

pub struct ExecutableGraph<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    edges: HashMap<String, String>,
    conditional: HashMap<String, Condition<S>>,
    checkpointer: Option<(Box<dyn Checkpointer<S>>, String)>,
    default_config: ExecutionConfig,
    entry: String,
}

impl<S: StateSchema> ExecutableGraph<S> {
    pub async fn invoke(&self, state: GraphState<S>) -> Result<GraphState<S>, WesichainError> {
        self.invoke_with_options(state, ExecutionOptions::default())
            .await
    }

    pub async fn invoke_with_options(
        &self,
        mut state: GraphState<S>,
        options: ExecutionOptions,
    ) -> Result<GraphState<S>, WesichainError> {
        let effective = self.default_config.merge(&options);
        let mut step_count = 0usize;
        let mut recent: VecDeque<String> = VecDeque::new();
        let mut current = self.entry.clone();

        loop {
            if let Some(max) = effective.max_steps {
                if step_count >= max {
                    return Err(WesichainError::Custom(
                        GraphError::MaxStepsExceeded {
                            max,
                            reached: step_count,
                        }
                        .to_string(),
                    ));
                }
            }
            step_count += 1;

            if effective.cycle_detection {
                if recent.len() == effective.cycle_window {
                    recent.pop_front();
                }
                recent.push_back(current.clone());
                let count = recent.iter().filter(|node| **node == current).count();
                if count >= 2 {
                    return Err(WesichainError::Custom(
                        GraphError::CycleDetected {
                            node: current.clone(),
                            recent: recent.iter().cloned().collect(),
                        }
                        .to_string(),
                    ));
                }
            }

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
