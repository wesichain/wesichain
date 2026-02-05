use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use futures::stream::{self, BoxStream, StreamExt};
use petgraph::graph::Graph;

use crate::{
    Checkpoint, Checkpointer, EdgeKind, ExecutionConfig, ExecutionOptions, GraphError, GraphEvent,
    GraphProgram, GraphState, NodeData, Observer, StateSchema, StateUpdate, END, START,
};
use wesichain_core::callbacks::{
    ensure_object, CallbackManager, RunContext, RunType, ToTraceInput, ToTraceOutput,
};
use wesichain_core::{Runnable, WesichainError};

pub type Condition<S> = Box<dyn Fn(&GraphState<S>) -> String + Send + Sync>;

pub struct GraphContext {
    pub remaining_steps: Option<usize>,
    pub observer: Option<Arc<dyn Observer>>,
    pub node_id: String,
}

#[async_trait::async_trait]
pub trait GraphNode<S: StateSchema>: Send + Sync {
    async fn invoke_with_context(
        &self,
        input: GraphState<S>,
        context: &GraphContext,
    ) -> Result<StateUpdate<S>, WesichainError>;
}

#[async_trait::async_trait]
impl<S, R> GraphNode<S> for R
where
    S: StateSchema,
    R: Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync,
{
    async fn invoke_with_context(
        &self,
        input: GraphState<S>,
        _context: &GraphContext,
    ) -> Result<StateUpdate<S>, WesichainError> {
        self.invoke(input).await
    }
}

pub struct GraphBuilder<S: StateSchema> {
    nodes: HashMap<String, Box<dyn GraphNode<S>>>,
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
        R: GraphNode<S> + 'static,
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
    nodes: HashMap<String, Box<dyn GraphNode<S>>>,
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

    pub fn stream_invoke(
        &self,
        state: GraphState<S>,
    ) -> BoxStream<'_, Result<GraphEvent, GraphError>> {
        if !self.nodes.contains_key(&self.entry) {
            return stream::iter(vec![Ok(GraphEvent::Error(GraphError::MissingNode {
                node: self.entry.clone(),
            }))])
            .boxed();
        }

        struct StreamState<S: StateSchema> {
            state: GraphState<S>,
            current: String,
            step_count: usize,
            recent: VecDeque<String>,
            pending: VecDeque<GraphEvent>,
            effective: ExecutionConfig,
            done: bool,
        }

        let state = StreamState {
            state,
            current: self.entry.clone(),
            step_count: 0,
            recent: VecDeque::new(),
            pending: VecDeque::new(),
            effective: self.default_config.merge(&ExecutionOptions::default()),
            done: false,
        };

        stream::unfold(state, move |mut ctx| async move {
            if let Some(event) = ctx.pending.pop_front() {
                return Some((Ok(event), ctx));
            }

            if ctx.done {
                return None;
            }

            if let Some(max) = ctx.effective.max_steps {
                if ctx.step_count >= max {
                    ctx.done = true;
                    return Some((
                        Ok(GraphEvent::Error(GraphError::MaxStepsExceeded {
                            max,
                            reached: ctx.step_count,
                        })),
                        ctx,
                    ));
                }
            }
            ctx.step_count += 1;

            if ctx.effective.cycle_detection {
                if ctx.recent.len() == ctx.effective.cycle_window {
                    ctx.recent.pop_front();
                }
                ctx.recent.push_back(ctx.current.clone());
                let count = ctx
                    .recent
                    .iter()
                    .filter(|node| **node == ctx.current)
                    .count();
                if count >= 2 {
                    ctx.done = true;
                    return Some((
                        Ok(GraphEvent::Error(GraphError::CycleDetected {
                            node: ctx.current.clone(),
                            recent: ctx.recent.iter().cloned().collect(),
                        })),
                        ctx,
                    ));
                }
            }

            if self.interrupt_before.contains(&ctx.current) {
                ctx.done = true;
                return Some((Ok(GraphEvent::Error(GraphError::Interrupted)), ctx));
            }

            let node = match self.nodes.get(&ctx.current) {
                Some(node) => node,
                None => {
                    ctx.done = true;
                    ctx.pending
                        .push_back(GraphEvent::Error(GraphError::InvalidEdge {
                            node: ctx.current.clone(),
                        }));
                    let event = ctx.pending.pop_front();
                    return event.map(|event| (Ok(event), ctx));
                }
            };

            ctx.pending.push_back(GraphEvent::NodeEnter {
                node: ctx.current.clone(),
            });

            let context = GraphContext {
                remaining_steps: ctx
                    .effective
                    .max_steps
                    .map(|max| max.saturating_sub(ctx.step_count.saturating_sub(1))),
                observer: None,
                node_id: ctx.current.clone(),
            };
            let update = match node.invoke_with_context(ctx.state.clone(), &context).await {
                Ok(update) => update,
                Err(err) => {
                    ctx.done = true;
                    ctx.pending
                        .push_back(GraphEvent::Error(GraphError::NodeFailed {
                            node: ctx.current.clone(),
                            source: Box::new(err),
                        }));
                    let event = ctx.pending.pop_front();
                    return event.map(|event| (Ok(event), ctx));
                }
            };

            ctx.state = GraphState::new(update.data);
            if let Some((checkpointer, thread_id)) = &self.checkpointer {
                let checkpoint = Checkpoint::new(
                    thread_id.clone(),
                    ctx.state.clone(),
                    ctx.step_count as u64,
                    ctx.current.clone(),
                );
                if let Err(err) = checkpointer.save(&checkpoint).await {
                    ctx.done = true;
                    ctx.pending.push_back(GraphEvent::Error(err));
                    let event = ctx.pending.pop_front();
                    return event.map(|event| (Ok(event), ctx));
                }
                ctx.pending.push_back(GraphEvent::CheckpointSaved {
                    node: ctx.current.clone(),
                });
            }

            ctx.pending.push_back(GraphEvent::NodeExit {
                node: ctx.current.clone(),
            });

            if self.interrupt_after.contains(&ctx.current) {
                ctx.done = true;
                ctx.pending
                    .push_back(GraphEvent::Error(GraphError::Interrupted));
                let event = ctx.pending.pop_front();
                return event.map(|event| (Ok(event), ctx));
            }

            if let Some(condition) = self.conditional.get(&ctx.current) {
                ctx.current = condition(&ctx.state);
                if !self.nodes.contains_key(&ctx.current) {
                    ctx.done = true;
                    ctx.pending
                        .push_back(GraphEvent::Error(GraphError::InvalidEdge {
                            node: ctx.current.clone(),
                        }));
                }
                let event = ctx.pending.pop_front();
                return event.map(|event| (Ok(event), ctx));
            }

            match self.edges.get(&ctx.current) {
                Some(next) => {
                    let next = next.clone();
                    if !self.nodes.contains_key(&next) {
                        ctx.done = true;
                        ctx.pending
                            .push_back(GraphEvent::Error(GraphError::InvalidEdge { node: next }));
                    } else {
                        ctx.current = next;
                    }
                }
                None => ctx.done = true,
            }

            let event = ctx.pending.pop_front();
            event.map(|event| (Ok(event), ctx))
        })
        .boxed()
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
        let observer = options.observer.clone().or_else(|| self.observer.clone());
        let mut callbacks: Option<(CallbackManager, RunContext)> = None;
        if let Some(run_config) = options.run_config {
            if let Some(manager) = run_config.callbacks {
                if !manager.is_noop() {
                    let name = run_config
                        .name_override
                        .unwrap_or_else(|| "graph_execution".to_string());
                    // Use `graph` for the root run; switch to `chain` if LangSmith UI lacks graph support.
                    let root = RunContext::root(
                        RunType::Graph,
                        name,
                        run_config.tags,
                        run_config.metadata,
                    );
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
                    let error = GraphError::MaxStepsExceeded {
                        max,
                        reached: step_count,
                    };
                    if let Some(obs) = &observer {
                        obs.on_error(&current, &error).await;
                    }
                    if let Some((manager, root)) = &callbacks {
                        let error_value = ensure_object(error.to_string().to_trace_output());
                        let duration_ms = root.start_instant.elapsed().as_millis();
                        manager.on_error(root, &error_value, duration_ms).await;
                    }
                    return Err(error);
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
                    let error = GraphError::CycleDetected {
                        node: current.clone(),
                        recent: recent.iter().cloned().collect(),
                    };
                    if let Some(obs) = &observer {
                        obs.on_error(&current, &error).await;
                    }
                    if let Some((manager, root)) = &callbacks {
                        let error_value = ensure_object(error.to_string().to_trace_output());
                        let duration_ms = root.start_instant.elapsed().as_millis();
                        manager.on_error(root, &error_value, duration_ms).await;
                    }
                    return Err(error);
                }
            }

            if self.interrupt_before.contains(&current) {
                let error = GraphError::Interrupted;
                if let Some(obs) = &observer {
                    obs.on_error(&current, &error).await;
                }
                if let Some((manager, root)) = &callbacks {
                    let error_value = ensure_object(error.to_string().to_trace_output());
                    let duration_ms = root.start_instant.elapsed().as_millis();
                    manager.on_error(root, &error_value, duration_ms).await;
                }
                return Err(error);
            }

            let node = match self.nodes.get(&current) {
                Some(node) => node,
                None => {
                    let error = GraphError::InvalidEdge {
                        node: current.clone(),
                    };
                    if let Some(obs) = &observer {
                        obs.on_error(&current, &error).await;
                    }
                    if let Some((manager, root)) = &callbacks {
                        let error_value = ensure_object(error.to_string().to_trace_output());
                        let duration_ms = root.start_instant.elapsed().as_millis();
                        manager.on_error(root, &error_value, duration_ms).await;
                    }
                    return Err(error);
                }
            };
            if let Some(obs) = &observer {
                let input_value = match serde_json::to_value(&state.data) {
                    Ok(value) => value,
                    Err(err) => {
                        let error = GraphError::NodeFailed {
                            node: current.clone(),
                            source: Box::new(err),
                        };
                        obs.on_error(&current, &error).await;
                        if let Some((manager, root)) = &callbacks {
                            let error_value = ensure_object(error.to_string().to_trace_output());
                            let duration_ms = root.start_instant.elapsed().as_millis();
                            manager.on_error(root, &error_value, duration_ms).await;
                        }
                        return Err(error);
                    }
                };
                obs.on_node_start(&current, &input_value).await;
            }
            let (manager, root) = match &callbacks {
                Some((manager, root)) => (Some(manager), Some(root)),
                None => (None, None),
            };
            let node_ctx = root.map(|root| root.child(RunType::Chain, current.clone()));
            if let (Some(manager), Some(node_ctx)) = (manager, &node_ctx) {
                let inputs = ensure_object(state.to_trace_input());
                manager.on_start(node_ctx, &inputs).await;
            }
            let context = GraphContext {
                remaining_steps: effective
                    .max_steps
                    .map(|max| max.saturating_sub(step_count.saturating_sub(1))),
                observer: observer.clone(),
                node_id: current.clone(),
            };
            let start = Instant::now();
            let update = match node.invoke_with_context(state, &context).await {
                Ok(update) => {
                    if let (Some(manager), Some(node_ctx)) = (manager, &node_ctx) {
                        let outputs = ensure_object(update.to_trace_output());
                        let duration_ms = node_ctx.start_instant.elapsed().as_millis();
                        manager.on_end(node_ctx, &outputs, duration_ms).await;
                    }
                    update
                }
                Err(err) => {
                    let error = GraphError::NodeFailed {
                        node: current.clone(),
                        source: Box::new(err),
                    };
                    if let Some(obs) = &observer {
                        obs.on_error(&current, &error).await;
                    }
                    let error_value = ensure_object(error.to_string().to_trace_output());
                    if let (Some(manager), Some(node_ctx)) = (manager, &node_ctx) {
                        let duration_ms = node_ctx.start_instant.elapsed().as_millis();
                        manager.on_error(node_ctx, &error_value, duration_ms).await;
                    }
                    if let (Some(manager), Some(root)) = (manager, root) {
                        let duration_ms = root.start_instant.elapsed().as_millis();
                        manager.on_error(root, &error_value, duration_ms).await;
                    }
                    return Err(error);
                }
            };
            let duration_ms = start.elapsed().as_millis();
            state = GraphState::new(update.data);
            if let Some(obs) = &observer {
                let output_value = match serde_json::to_value(&state.data) {
                    Ok(value) => value,
                    Err(err) => {
                        let error = GraphError::NodeFailed {
                            node: current.clone(),
                            source: Box::new(err),
                        };
                        obs.on_error(&current, &error).await;
                        if let Some((manager, root)) = &callbacks {
                            let error_value = ensure_object(error.to_string().to_trace_output());
                            let duration_ms = root.start_instant.elapsed().as_millis();
                            manager.on_error(root, &error_value, duration_ms).await;
                        }
                        return Err(error);
                    }
                };
                obs.on_node_end(&current, &output_value, duration_ms).await;
            }
            if let Some((checkpointer, thread_id)) = &self.checkpointer {
                let checkpoint = Checkpoint::new(
                    thread_id.clone(),
                    state.clone(),
                    step_count as u64,
                    current.clone(),
                );
                if let Err(err) = checkpointer.save(&checkpoint).await {
                    if let Some(obs) = &observer {
                        obs.on_error(&current, &err).await;
                    }
                    if let Some((manager, root)) = &callbacks {
                        let error_value = ensure_object(err.to_string().to_trace_output());
                        let duration_ms = root.start_instant.elapsed().as_millis();
                        manager.on_error(root, &error_value, duration_ms).await;
                    }
                    return Err(err);
                }
                if let Some(obs) = &observer {
                    obs.on_checkpoint_saved(&current).await;
                }
            }

            if self.interrupt_after.contains(&current) {
                let error = GraphError::Interrupted;
                if let Some(obs) = &observer {
                    obs.on_error(&current, &error).await;
                }
                if let Some((manager, root)) = &callbacks {
                    let error_value = ensure_object(error.to_string().to_trace_output());
                    let duration_ms = root.start_instant.elapsed().as_millis();
                    manager.on_error(root, &error_value, duration_ms).await;
                }
                return Err(error);
            }
            if let Some(condition) = self.conditional.get(&current) {
                let next = condition(&state);
                if !self.nodes.contains_key(&next) {
                    let error = GraphError::InvalidEdge { node: next.clone() };
                    if let Some(obs) = &observer {
                        obs.on_error(&current, &error).await;
                    }
                    if let Some((manager, root)) = &callbacks {
                        let error_value = ensure_object(error.to_string().to_trace_output());
                        let duration_ms = root.start_instant.elapsed().as_millis();
                        manager.on_error(root, &error_value, duration_ms).await;
                    }
                    return Err(error);
                }
                current = next;
                continue;
            }
            match self.edges.get(&current) {
                Some(next) => {
                    let next = next.clone();
                    if !self.nodes.contains_key(&next) {
                        let error = GraphError::InvalidEdge { node: next.clone() };
                        if let Some(obs) = &observer {
                            obs.on_error(&current, &error).await;
                        }
                        if let Some((manager, root)) = &callbacks {
                            let error_value = ensure_object(error.to_string().to_trace_output());
                            let duration_ms = root.start_instant.elapsed().as_millis();
                            manager.on_error(root, &error_value, duration_ms).await;
                        }
                        return Err(error);
                    }
                    current = next;
                }
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
