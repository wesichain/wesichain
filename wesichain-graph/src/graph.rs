use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use petgraph::graph::Graph;

use crate::{
    Checkpoint, Checkpointer, EdgeKind, ExecutionConfig, ExecutionOptions, GraphError,
    GraphProgram, GraphState, NodeData, Observer, StateSchema, StateUpdate, END, START,
};
use wesichain_core::{Runnable, WesichainError};

pub type Condition<S> = Box<dyn Fn(&GraphState<S>) -> String + Send + Sync>;

pub struct GraphBuilder<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    edges: HashMap<String, String>,
    conditional: HashMap<String, Condition<S>>,
    checkpointer: Option<(Box<dyn Checkpointer<S>>, String)>,
    observer: Option<Arc<dyn Observer>>,
    default_config: ExecutionConfig,
    entry: Option<String>,
    interrupt_before: Vec<String>,
    interrupt_after: Vec<String>,
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
            observer: None,
            default_config: ExecutionConfig::default(),
            entry: None,
            interrupt_before: Vec::new(),
            interrupt_after: Vec::new(),
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

    pub fn with_observer(mut self, observer: Arc<dyn Observer>) -> Self {
        self.observer = Some(observer);
        self
    }

    pub fn with_default_config(mut self, config: ExecutionConfig) -> Self {
        self.default_config = config;
        self
    }

    pub fn with_interrupt_before<I, S2>(mut self, nodes: I) -> Self
    where
        I: IntoIterator<Item = S2>,
        S2: Into<String>,
    {
        self.interrupt_before = nodes.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_interrupt_after<I, S2>(mut self, nodes: I) -> Self
    where
        I: IntoIterator<Item = S2>,
        S2: Into<String>,
    {
        self.interrupt_after = nodes.into_iter().map(Into::into).collect();
        self
    }

    pub fn build(self) -> ExecutableGraph<S> {
        ExecutableGraph {
            nodes: self.nodes,
            edges: self.edges,
            conditional: self.conditional,
            checkpointer: self.checkpointer,
            observer: self.observer,
            default_config: self.default_config,
            entry: self.entry.expect("entry"),
            interrupt_before: self.interrupt_before,
            interrupt_after: self.interrupt_after,
        }
    }

    pub fn build_program(self) -> GraphProgram<S> {
        let GraphBuilder { nodes, edges, .. } = self;
        let mut graph = Graph::new();
        let mut name_to_index = HashMap::new();

        for (name, runnable) in nodes {
            let index = graph.add_node(NodeData {
                name: name.clone(),
                runnable,
            });
            name_to_index.insert(name, index);
        }

        for (from, to) in edges.iter() {
            if from == START || to == END {
                continue;
            }
            if let (Some(from_idx), Some(to_idx)) = (name_to_index.get(from), name_to_index.get(to))
            {
                graph.add_edge(*from_idx, *to_idx, EdgeKind::Default);
            }
        }

        GraphProgram::new(graph, name_to_index)
    }
}

pub struct ExecutableGraph<S: StateSchema> {
    nodes: HashMap<String, Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>>,
    edges: HashMap<String, String>,
    conditional: HashMap<String, Condition<S>>,
    checkpointer: Option<(Box<dyn Checkpointer<S>>, String)>,
    observer: Option<Arc<dyn Observer>>,
    default_config: ExecutionConfig,
    entry: String,
    interrupt_before: Vec<String>,
    interrupt_after: Vec<String>,
}

impl<S: StateSchema> ExecutableGraph<S> {
    pub async fn invoke_graph(&self, state: GraphState<S>) -> Result<GraphState<S>, GraphError> {
        self.invoke_graph_with_options(state, ExecutionOptions::default())
            .await
    }

    pub async fn invoke_graph_with_options(
        &self,
        mut state: GraphState<S>,
        options: ExecutionOptions,
    ) -> Result<GraphState<S>, GraphError> {
        if !self.nodes.contains_key(&self.entry) {
            return Err(GraphError::MissingNode {
                node: self.entry.clone(),
            });
        }
        let effective = self.default_config.merge(&options);
        let mut step_count = 0usize;
        let mut recent: VecDeque<String> = VecDeque::new();
        let mut current = self.entry.clone();

        loop {
            if let Some(max) = effective.max_steps {
                if step_count >= max {
                    return Err(GraphError::MaxStepsExceeded {
                        max,
                        reached: step_count,
                    });
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
                    return Err(GraphError::CycleDetected {
                        node: current.clone(),
                        recent: recent.iter().cloned().collect(),
                    });
                }
            }

            if self.interrupt_before.contains(&current) {
                return Err(GraphError::Interrupted);
            }

            let node = self
                .nodes
                .get(&current)
                .ok_or_else(|| GraphError::InvalidEdge {
                    node: current.clone(),
                })?;
            if let Some(observer) = &self.observer {
                observer.on_node_enter(&current);
            }
            let update = node.invoke(state).await.map_err(|err| {
                if let Some(observer) = &self.observer {
                    observer.on_error(&current, &err.to_string());
                }
                GraphError::NodeFailed {
                    node: current.clone(),
                    source: Box::new(err),
                }
            })?;
            state = GraphState::new(update.data);
            if let Some((checkpointer, thread_id)) = &self.checkpointer {
                let checkpoint = Checkpoint::new(
                    thread_id.clone(),
                    state.clone(),
                    step_count as u64,
                    current.clone(),
                );
                if let Err(err) = checkpointer.save(&checkpoint).await {
                    if let Some(observer) = &self.observer {
                        observer.on_error(&current, &err.to_string());
                    }
                    return Err(err);
                }
                if let Some(observer) = &self.observer {
                    observer.on_checkpoint_saved(&current);
                }
            }
            if let Some(observer) = &self.observer {
                observer.on_node_exit(&current);
            }

            if self.interrupt_after.contains(&current) {
                return Err(GraphError::Interrupted);
            }
            if let Some(condition) = self.conditional.get(&current) {
                current = condition(&state);
                if !self.nodes.contains_key(&current) {
                    return Err(GraphError::InvalidEdge { node: current });
                }
                continue;
            }
            match self.edges.get(&current) {
                Some(next) => {
                    let next = next.clone();
                    if !self.nodes.contains_key(&next) {
                        return Err(GraphError::InvalidEdge { node: next });
                    }
                    current = next;
                }
                None => break,
            }
        }
        Ok(state)
    }

    pub async fn invoke(&self, state: GraphState<S>) -> Result<GraphState<S>, WesichainError> {
        self.invoke_graph(state)
            .await
            .map_err(|err| WesichainError::Custom(err.to_string()))
    }

    pub async fn invoke_with_options(
        &self,
        state: GraphState<S>,
        options: ExecutionOptions,
    ) -> Result<GraphState<S>, WesichainError> {
        self.invoke_graph_with_options(state, options)
            .await
            .map_err(|err| WesichainError::Custom(err.to_string()))
    }
}
