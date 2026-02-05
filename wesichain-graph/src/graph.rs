use std::collections::{HashMap, VecDeque};

use crate::{
    Checkpoint, Checkpointer, ExecutionConfig, ExecutionOptions, GraphError, GraphState,
    StateSchema, StateUpdate,
};
use wesichain_core::callbacks::{
    ensure_object, CallbackManager, RunContext, RunType, ToTraceInput, ToTraceOutput,
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
        let mut callbacks: Option<(CallbackManager, RunContext)> = None;
        if let Some(run_config) = options.run_config {
            if let Some(manager) = run_config.callbacks {
                if !manager.is_noop() {
                    let name = run_config
                        .name_override
                        .unwrap_or_else(|| "graph_execution".to_string());
                    // Use `graph` for the root run; switch to `chain` if LangSmith UI lacks graph support.
                    let root = RunContext::root(RunType::Graph, name, run_config.tags, run_config.metadata);
                    let inputs = ensure_object(state.to_trace_input());
                    manager.on_start(&root, &inputs).await;
                    callbacks = Some((manager, root));
                }
            }
        }
        let mut step_count = 0usize;
        let mut recent: VecDeque<String> = VecDeque::new();
        let mut current = self.entry.clone();

        loop {
            if let Some(max) = effective.max_steps {
                if step_count >= max {
                    let err = WesichainError::Custom(
                        GraphError::MaxStepsExceeded {
                            max,
                            reached: step_count,
                        }
                        .to_string(),
                    );
                    if let Some((manager, root)) = &callbacks {
                        let error = ensure_object(err.to_string().to_trace_output());
                        let duration_ms = root.start_instant.elapsed().as_millis();
                        manager.on_error(root, &error, duration_ms).await;
                    }
                    return Err(err);
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
                    let err = WesichainError::Custom(
                        GraphError::CycleDetected {
                            node: current.clone(),
                            recent: recent.iter().cloned().collect(),
                        }
                        .to_string(),
                    );
                    if let Some((manager, root)) = &callbacks {
                        let error = ensure_object(err.to_string().to_trace_output());
                        let duration_ms = root.start_instant.elapsed().as_millis();
                        manager.on_error(root, &error, duration_ms).await;
                    }
                    return Err(err);
                }
            }

            let node = self.nodes.get(&current).expect("node");
            let (manager, root) = match &callbacks {
                Some((manager, root)) => (Some(manager), Some(root)),
                None => (None, None),
            };
            let node_ctx = root.map(|root| root.child(RunType::Chain, current.clone()));
            if let (Some(manager), Some(node_ctx)) = (manager, &node_ctx) {
                let inputs = ensure_object(state.to_trace_input());
                manager.on_start(node_ctx, &inputs).await;
            }

            let update = match node.invoke(state).await {
                Ok(update) => {
                    if let (Some(manager), Some(node_ctx)) = (manager, &node_ctx) {
                        let outputs = ensure_object(update.to_trace_output());
                        let duration_ms = node_ctx.start_instant.elapsed().as_millis();
                        manager.on_end(node_ctx, &outputs, duration_ms).await;
                    }
                    update
                }
                Err(err) => {
                    let error_value = ensure_object(err.to_string().to_trace_output());
                    if let (Some(manager), Some(node_ctx)) = (manager, &node_ctx) {
                        let duration_ms = node_ctx.start_instant.elapsed().as_millis();
                        manager.on_error(node_ctx, &error_value, duration_ms).await;
                    }
                    if let (Some(manager), Some(root)) = (manager, root) {
                        let duration_ms = root.start_instant.elapsed().as_millis();
                        manager.on_error(root, &error_value, duration_ms).await;
                    }
                    return Err(err);
                }
            };
            state = GraphState::new(update.data);
            if let Some((checkpointer, thread_id)) = &self.checkpointer {
                let checkpoint = Checkpoint::new(thread_id.clone(), state.clone());
                if let Err(err) = checkpointer.save(&checkpoint).await {
                    let error = WesichainError::CheckpointFailed(err.to_string());
                    if let Some((manager, root)) = &callbacks {
                        let error_value = ensure_object(error.to_string().to_trace_output());
                        let duration_ms = root.start_instant.elapsed().as_millis();
                        manager.on_error(root, &error_value, duration_ms).await;
                    }
                    return Err(error);
                }
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
        if let Some((manager, root)) = &callbacks {
            let outputs = ensure_object(state.to_trace_output());
            let duration_ms = root.start_instant.elapsed().as_millis();
            manager.on_end(root, &outputs, duration_ms).await;
        }
        Ok(state)
    }
}
