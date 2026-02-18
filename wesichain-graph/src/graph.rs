use ahash::RandomState;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::sync::Arc;

use chrono::Utc;
use futures::stream::{self, BoxStream, StreamExt};
use petgraph::graph::Graph;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::observer::ObserverCallbackAdapter;
use crate::{
    Checkpoint, Checkpointer, EdgeKind, ExecutionConfig, ExecutionOptions, GraphError, GraphEvent,
    GraphProgram, GraphState, NodeData, Observer, StateSchema, StateUpdate, END, START,
};
use serde_json::json;
use wesichain_core::{
    ensure_object, AgentEvent, CallbackManager, RunContext, RunType, Runnable, ToTraceInput,
    ToTraceOutput, WesichainError,
};

pub type Condition<S> = Box<dyn Fn(&GraphState<S>) -> Vec<String> + Send + Sync>;

pub struct GraphContext {
    pub remaining_steps: Option<usize>,
    pub observer: Option<Arc<dyn Observer>>,
    pub node_id: String,
}

async fn emit_status_event(
    sender: &Option<mpsc::Sender<AgentEvent>>,
    step: &mut usize,
    thread_id: &str,
    stage: impl Into<String>,
    message: impl Into<String>,
) {
    if let Some(sender) = sender {
        *step += 1;
        let _ = sender
            .send(AgentEvent::Status {
                stage: stage.into(),
                message: message.into(),
                step: *step,
                thread_id: thread_id.to_string(),
            })
            .await;
    }
}

async fn emit_error_event(
    sender: &Option<mpsc::Sender<AgentEvent>>,
    step: &mut usize,
    message: impl Into<String>,
    source: Option<String>,
) {
    if let Some(sender) = sender {
        *step += 1;
        let _ = sender
            .send(AgentEvent::Error {
                message: message.into(),
                step: *step,
                recoverable: false,
                source,
            })
            .await;
    }
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
    nodes: HashMap<String, Arc<dyn GraphNode<S>>>,
    edges: HashMap<String, Vec<String>>,
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
        self.nodes.insert(name.to_string(), Arc::new(node));
        self
    }

    pub fn set_entry(mut self, name: &str) -> Self {
        self.entry = Some(name.to_string());
        self
    }

    pub fn add_edge(mut self, from: &str, to: &str) -> Self {
        self.edges
            .entry(from.to_string())
            .or_default()
            .push(to.to_string());
        self
    }

    pub fn add_edges(mut self, from: &str, targets: &[&str]) -> Self {
        let entry = self.edges.entry(from.to_string()).or_default();
        for target in targets {
            entry.push(target.to_string());
        }
        self
    }

    pub fn add_conditional_edge<F>(mut self, from: &str, condition: F) -> Self
    where
        F: Fn(&GraphState<S>) -> Vec<String> + Send + Sync + 'static,
    {
        self.conditional
            .insert(from.to_string(), Box::new(condition));
        self
    }
    #[deprecated(since = "0.3.0", note = "Use `with_default_config` instead")]
    pub fn with_config(mut self, config: ExecutionConfig) -> Self {
        self.default_config = config;
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

        for (from, targets) in edges.iter() {
            if from == START {
                continue;
            }
            if let Some(from_idx) = name_to_index.get(from) {
                for to in targets {
                    if to == END {
                        continue;
                    }
                    if let Some(to_idx) = name_to_index.get(to) {
                        graph.add_edge(*from_idx, *to_idx, EdgeKind::Default);
                    }
                }
            }
        }

        GraphProgram::new(graph, name_to_index)
    }
}

fn stable_hash<T: Hash + ?Sized>(t: &T) -> u64 {
    RandomState::with_seeds(0x517cc1b727220a95, 0x6ed9eba1999cd92d, 0, 0).hash_one(t)
}

pub struct ExecutableGraph<S: StateSchema> {
    nodes: HashMap<String, Arc<dyn GraphNode<S>>>,
    edges: HashMap<String, Vec<String>>,
    conditional: HashMap<String, Condition<S>>,
    checkpointer: Option<(Box<dyn Checkpointer<S>>, String)>,
    observer: Option<Arc<dyn Observer>>,
    default_config: ExecutionConfig,
    entry: String,
    interrupt_before: Vec<String>,
    interrupt_after: Vec<String>,
}

impl<S: StateSchema<Update = S>> ExecutableGraph<S> {
    pub async fn invoke_graph(&self, state: GraphState<S>) -> Result<GraphState<S>, GraphError> {
        self.invoke_graph_with_options(state, ExecutionOptions::default())
            .await
    }

    pub fn stream_invoke(
        &self,
        state: GraphState<S>,
    ) -> BoxStream<'_, Result<GraphEvent<S>, GraphError>> {
        self.stream_invoke_with_options(state, ExecutionOptions::default())
    }

    pub fn stream_invoke_with_options(
        &self,
        state: GraphState<S>,
        options: ExecutionOptions,
    ) -> BoxStream<'_, Result<GraphEvent<S>, GraphError>> {
        let checkpoint_thread_id = options.checkpoint_thread_id.clone().or_else(|| {
            self.checkpointer
                .as_ref()
                .map(|(_, thread_id)| thread_id.clone())
        });

        // Initialize Callbacks and Observer (Unified)
        let observer = options.observer.clone().or_else(|| self.observer.clone());
        let mut run_config = options.run_config.clone().unwrap_or_default();

        if let Some(obs) = observer {
            let adapter = Arc::new(ObserverCallbackAdapter(obs));
            let handlers = if let Some(mut manager) = run_config.callbacks.take() {
                // Merge the adapter into the existing CallbackManager
                manager.add_handler(adapter);
                manager
            } else {
                CallbackManager::new(vec![adapter])
            };
            run_config.callbacks = Some(handlers);
        }

        let run_config_option = Some(run_config);

        // We need to run initialization async to call on_start
        // Since stream::unfold expects an initial state, we'll use a wrapper enum or
        // handle initialization in the first step of the loop.
        // Or better yet, we can't easily do async setup *outside* the stream if we return a stream immediately.
        // So we'll trigger the start events in the first iteration.

        struct StreamState<S: StateSchema> {
            state: GraphState<S>,
            step_count: usize,
            recent: VecDeque<String>,
            pending_events: VecDeque<GraphEvent<S>>,
            effective: ExecutionConfig,
            queue: VecDeque<(String, u64)>,
            join_set: JoinSet<(String, Result<StateUpdate<S>, WesichainError>, u64)>,
            start_time: std::time::Instant,
            visit_counts: HashMap<String, u32>,
            path_visits: HashMap<(String, u64), u32>,
            // Unified fields
            active_tasks: HashSet<(String, u64)>,
            callbacks: Option<(CallbackManager, RunContext)>,
            callback_nodes: HashMap<(String, u64), RunContext>,
            agent_event_sender: Option<mpsc::Sender<AgentEvent>>,
            agent_event_thread_id: String,
            agent_event_step: usize,
            checkpoint_thread_id: Option<String>,
            initialized: bool,
            run_config: Option<wesichain_core::RunConfig>, // Store for delayed init
        }

        if !self.nodes.contains_key(&self.entry) {
            return stream::iter(vec![Ok(GraphEvent::Error(GraphError::MissingNode {
                node: self.entry.clone(),
            }))])
            .boxed();
        }

        let effective = self.default_config.merge(&options);

        let agent_event_thread_id = options
            .agent_event_thread_id
            .clone()
            .or_else(|| checkpoint_thread_id.clone())
            .unwrap_or_else(|| "graph".to_string());

        let initial_queue = options
            .initial_queue
            .clone()
            .map(VecDeque::from)
            .unwrap_or_else(|| VecDeque::from([(self.entry.clone(), 0)]));

        let initial_step = options.initial_step.unwrap_or(0);

        let stream_state = StreamState {
            state,
            step_count: initial_step,
            recent: VecDeque::new(),
            pending_events: VecDeque::new(),
            effective,
            queue: initial_queue,
            join_set: JoinSet::new(),
            start_time: std::time::Instant::now(),
            visit_counts: HashMap::new(),
            path_visits: HashMap::new(),
            active_tasks: HashSet::new(),
            callbacks: None, // Will init in loop
            callback_nodes: HashMap::new(),
            agent_event_sender: options.agent_event_sender,
            agent_event_thread_id,
            agent_event_step: 0,
            checkpoint_thread_id,
            initialized: false,
            run_config: run_config_option,
        };

        stream::unfold(stream_state, move |mut ctx| async move {
            loop {
                // 1. delayed initialization (on first poll)
                if !ctx.initialized {
                    ctx.initialized = true;

                    // Initialize callbacks
                    if let Some(run_config) = ctx.run_config.take() {
                        if let Some(manager) = run_config.callbacks {
                            if !manager.is_noop() {
                                let name = run_config
                                    .name_override
                                    .unwrap_or_else(|| "graph_execution".to_string());
                                let root = RunContext::root(
                                    RunType::Graph,
                                    name,
                                    run_config.tags,
                                    run_config.metadata,
                                );
                                let inputs = ensure_object(ctx.state.to_trace_input());
                                manager.on_start(&root, &inputs).await;
                                ctx.callbacks = Some((manager, root));
                            }
                        }
                    }
                }

                // 2. Emit pending events
                if let Some(event) = ctx.pending_events.pop_front() {
                    return Some((Ok(event), ctx));
                }

                // 3. Process Queue
                if let Some((current, path_id)) = ctx.queue.pop_front() {
                    // Safety Checks
                    // Global Timer
                    if let Some(duration) = ctx.effective.max_duration {
                        if ctx.start_time.elapsed() > duration {
                            let error = GraphError::Timeout {
                                node: "global".to_string(),
                                elapsed: ctx.start_time.elapsed(),
                            };
                            // callbacks error
                            if let Some((manager, root)) = &ctx.callbacks {
                                let error_value =
                                    ensure_object(error.to_string().to_trace_output());
                                let duration_ms = root.start_instant.elapsed().as_millis();
                                manager.on_error(root, &error_value, duration_ms).await;
                            }

                            ctx.join_set.shutdown().await;
                            ctx.pending_events.push_back(GraphEvent::Error(error));
                            continue;
                        }
                    }

                    // Max Steps
                    if let Some(max) = ctx.effective.max_steps {
                        if ctx.step_count >= max {
                            let error = GraphError::MaxStepsExceeded {
                                max,
                                reached: ctx.step_count,
                            };
                            if let Some((manager, root)) = &ctx.callbacks {
                                let error_value =
                                    ensure_object(error.to_string().to_trace_output());
                                let duration_ms = root.start_instant.elapsed().as_millis();
                                manager.on_error(root, &error_value, duration_ms).await;
                            }
                            ctx.join_set.shutdown().await;
                            ctx.pending_events.push_back(GraphEvent::Error(error));
                            continue;
                        }
                    }

                    // Max Visits
                    if let Some(max_visits) = ctx.effective.max_visits {
                        let count = ctx.visit_counts.entry(current.clone()).or_insert(0);
                        *count += 1;
                        if *count > max_visits {
                            let error = GraphError::MaxVisitsExceeded {
                                node: current.clone(),
                                max: max_visits,
                            };
                            if let Some((manager, root)) = &ctx.callbacks {
                                let error_value =
                                    ensure_object(error.to_string().to_trace_output());
                                let duration_ms = root.start_instant.elapsed().as_millis();
                                manager.on_error(root, &error_value, duration_ms).await;
                            }
                            ctx.join_set.shutdown().await;
                            ctx.pending_events.push_back(GraphEvent::Error(error));
                            continue;
                        }
                    }

                    // Path loops
                    if let Some(max_loops) = ctx.effective.max_loop_iterations {
                        let key = (current.clone(), path_id);
                        let count = ctx.path_visits.entry(key).or_insert(0);
                        *count += 1;
                        if *count > max_loops {
                            let error = GraphError::MaxLoopIterationsExceeded {
                                node: current.clone(),
                                max: max_loops,
                                path_id,
                            };
                            if let Some((manager, root)) = &ctx.callbacks {
                                let error_value =
                                    ensure_object(error.to_string().to_trace_output());
                                let duration_ms = root.start_instant.elapsed().as_millis();
                                manager.on_error(root, &error_value, duration_ms).await;
                            }
                            ctx.join_set.shutdown().await;
                            ctx.pending_events.push_back(GraphEvent::Error(error));
                            continue;
                        }
                    }

                    ctx.step_count += 1;

                    // Cycle detection
                    if ctx.effective.cycle_detection {
                        if ctx.recent.len() == ctx.effective.cycle_window {
                            ctx.recent.pop_front();
                        }
                        ctx.recent.push_back(current.clone());
                        let count = ctx.recent.iter().filter(|node| **node == current).count();
                        if count >= 2 {
                            let error = GraphError::CycleDetected {
                                node: current.clone(),
                                recent: ctx.recent.iter().cloned().collect(),
                            };
                            if let Some((manager, root)) = &ctx.callbacks {
                                let error_value =
                                    ensure_object(error.to_string().to_trace_output());
                                let duration_ms = root.start_instant.elapsed().as_millis();
                                manager.on_error(root, &error_value, duration_ms).await;
                            }
                            ctx.join_set.shutdown().await;
                            ctx.pending_events.push_back(GraphEvent::Error(error));
                            continue;
                        }
                    }

                    // Interrupt Before
                    if ctx.effective.interrupt_before.contains(&current)
                        || self.interrupt_before.contains(&current)
                    {
                        let error = GraphError::Interrupted;
                        if let Some((manager, root)) = &ctx.callbacks {
                            let error_value = ensure_object(error.to_string().to_trace_output());
                            let duration_ms = root.start_instant.elapsed().as_millis();
                            manager.on_error(root, &error_value, duration_ms).await;
                        }

                        // Save checkpoint on interrupt
                        if let (Some((checkpointer, _)), Some(thread_id)) = (
                            self.checkpointer.as_ref(),
                            ctx.checkpoint_thread_id.as_deref(),
                        ) {
                            let mut full_queue = ctx.queue.iter().cloned().collect::<Vec<_>>();
                            full_queue.push((current.clone(), path_id));
                            full_queue.extend(ctx.active_tasks.iter().cloned());

                            let checkpoint = Checkpoint::new(
                                thread_id.to_string(),
                                ctx.state.clone(),
                                ctx.step_count as u64,
                                current.clone(),
                                full_queue,
                            );
                            if let Err(e) = checkpointer.save(&checkpoint).await {
                                let graph_err = GraphError::from(e);
                                if let Some((manager, root)) = &ctx.callbacks {
                                    let error_value =
                                        ensure_object(graph_err.to_string().to_trace_output());
                                    let duration_ms = root.start_instant.elapsed().as_millis();
                                    manager.on_error(root, &error_value, duration_ms).await;
                                }
                                ctx.pending_events.push_back(GraphEvent::Error(graph_err));
                            } else {
                                ctx.pending_events.push_back(GraphEvent::CheckpointSaved {
                                    node: current.clone(),
                                    timestamp: Utc::now().timestamp_millis() as u64,
                                });
                                if let Some((manager, root)) = &ctx.callbacks {
                                    // Checkpoint saved event
                                    manager
                                        .on_event(
                                            root,
                                            "checkpoint_saved",
                                            &json!({"node_id": current}),
                                        )
                                        .await;
                                }
                            }
                        }

                        ctx.join_set.shutdown().await;
                        ctx.pending_events.push_back(GraphEvent::Error(error));
                        continue;
                    }

                    // Get Node
                    let node = match self.nodes.get(&current) {
                        Some(node) => node.clone(),
                        None => {
                            let error = GraphError::InvalidEdge {
                                node: current.clone(),
                            };
                            // observers...
                            ctx.pending_events.push_back(GraphEvent::Error(error));
                            continue;
                        }
                    };

                    // Side Effects: Node Start
                    emit_status_event(
                        &ctx.agent_event_sender,
                        &mut ctx.agent_event_step,
                        &ctx.agent_event_thread_id,
                        "node_start",
                        format!("Starting node {current}"),
                    )
                    .await;

                    if let Some((manager, root)) = &ctx.callbacks {
                        let node_ctx = root.child(RunType::Chain, current.clone());
                        let node_inputs = ensure_object(ctx.state.to_trace_input());
                        manager.on_start(&node_ctx, &node_inputs).await;
                        ctx.callback_nodes
                            .insert((current.clone(), path_id), node_ctx);
                    }

                    ctx.pending_events.push_back(GraphEvent::NodeEnter {
                        node: current.clone(),
                        timestamp: Utc::now().timestamp_millis() as u64,
                    });

                    // Prepare Node Execution
                    let input_state = ctx.state.clone();
                    // We need a node-specific context
                    let node_ctx_obs = None; // Observer removed from StreamState
                    let node_id = current.clone();
                    let effective_config_spawn = ctx.effective.clone();
                    let remaining = effective_config_spawn
                        .max_steps
                        .map(|m| m.saturating_sub(ctx.step_count)); // approximate

                    let context = GraphContext {
                        remaining_steps: remaining,
                        observer: node_ctx_obs,
                        node_id: node_id.clone(),
                    };

                    ctx.active_tasks.insert((current.clone(), path_id));

                    // Spawn
                    ctx.join_set.spawn(async move {
                        let future = node.invoke_with_context(input_state, &context);
                        let result = if let Some(timeout) = effective_config_spawn.node_timeout {
                            match tokio::time::timeout(timeout, future).await {
                                Ok(res) => res,
                                Err(_) => Err(WesichainError::Custom(format!(
                                    "Node {} timed out after {:?}",
                                    node_id, timeout
                                ))),
                            }
                        } else {
                            future.await
                        };
                        (current, result, path_id)
                    });

                    continue; // Loop back to pick up next event or task
                }

                // 4. Process Completed Tasks
                if !ctx.join_set.is_empty() {
                    if let Some(join_res) = ctx.join_set.join_next().await {
                        let (current, invoke_res, path_id) = match join_res {
                            Ok(r) => r,
                            Err(err) => {
                                let error = GraphError::System(err.to_string());
                                ctx.join_set.shutdown().await;
                                ctx.pending_events.push_back(GraphEvent::Error(error));
                                continue;
                            }
                        };

                        ctx.active_tasks.remove(&(current.clone(), path_id));

                        match invoke_res {
                            Ok(update) => {
                                // Node Success
                                let output_debug =
                                    serde_json::to_string(&update).unwrap_or_default();
                                ctx.state = ctx.state.apply_update(update.clone());

                                ctx.pending_events.push_back(GraphEvent::NodeFinished {
                                    node: current.clone(),
                                    output: output_debug,
                                    timestamp: Utc::now().timestamp_millis() as u64,
                                });

                                // CRITICAL: Emit StateUpdate for invoke_graph consumers
                                ctx.pending_events
                                    .push_back(GraphEvent::StateUpdate(update));

                                // Callbacks end
                                if let Some((manager, _root)) = &ctx.callbacks {
                                    if let Some(node_ctx) =
                                        ctx.callback_nodes.remove(&(current.clone(), path_id))
                                    {
                                        let node_outputs =
                                            ensure_object(ctx.state.to_trace_output());
                                        let duration_ms =
                                            node_ctx.start_instant.elapsed().as_millis();
                                        manager.on_end(&node_ctx, &node_outputs, duration_ms).await;
                                    }
                                }
                                // Observer end (handled by callbacks)
                                emit_status_event(
                                    &ctx.agent_event_sender,
                                    &mut ctx.agent_event_step,
                                    &ctx.agent_event_thread_id,
                                    "node_end",
                                    format!("Completed node {current}"),
                                )
                                .await;

                                ctx.pending_events.push_back(GraphEvent::NodeExit {
                                    node: current.clone(),
                                    timestamp: Utc::now().timestamp_millis() as u64,
                                });

                                // 4c. Route Next (moved before Checkpoint)
                                if let Some(condition) = self.conditional.get(&current) {
                                    let targets = condition(&ctx.state);
                                    let next_paths: Vec<(String, u64)> = if targets.len() > 1 {
                                        targets
                                            .into_iter()
                                            .map(|t| {
                                                if t == END {
                                                    (t, path_id)
                                                } else {
                                                    let h = stable_hash(&(path_id, &t));
                                                    (t, h)
                                                }
                                            })
                                            .collect()
                                    } else {
                                        targets.into_iter().map(|t| (t, path_id)).collect()
                                    };

                                    for (next, next_path_id) in next_paths {
                                        if next == END {
                                            continue;
                                        }
                                        if !self.nodes.contains_key(&next) {
                                            // Error
                                            let error =
                                                GraphError::InvalidEdge { node: next.clone() };
                                            ctx.pending_events.push_back(GraphEvent::Error(error));
                                            ctx.join_set.shutdown().await;
                                            continue; // Outer loop continues, catches next event
                                        }
                                        ctx.queue.push_back((next, next_path_id));
                                    }
                                } else if let Some(targets) = self.edges.get(&current) {
                                    let next_paths: Vec<(String, u64)> = if targets.len() > 1 {
                                        targets
                                            .iter()
                                            .map(|t| {
                                                if *t == END {
                                                    (t.clone(), path_id)
                                                } else {
                                                    (t.clone(), stable_hash(&(path_id, t)))
                                                }
                                            })
                                            .collect()
                                    } else {
                                        targets.iter().cloned().map(|t| (t, path_id)).collect()
                                    };

                                    for (next, next_path_id) in next_paths {
                                        if next == END {
                                            continue;
                                        }
                                        if !self.nodes.contains_key(&next) {
                                            let error =
                                                GraphError::InvalidEdge { node: next.clone() };
                                            ctx.pending_events.push_back(GraphEvent::Error(error));
                                            ctx.join_set.shutdown().await;
                                            continue;
                                        }
                                        ctx.queue.push_back((next, next_path_id));
                                    }
                                }

                                // 4a. Checkpoint
                                if let (Some((checkpointer, _)), Some(thread_id)) = (
                                    self.checkpointer.as_ref(),
                                    ctx.checkpoint_thread_id.as_deref(),
                                ) {
                                    let mut full_queue =
                                        ctx.queue.iter().cloned().collect::<Vec<_>>();
                                    full_queue.extend(ctx.active_tasks.iter().cloned());

                                    let checkpoint = Checkpoint::new(
                                        thread_id.to_string(),
                                        ctx.state.clone(),
                                        ctx.step_count as u64,
                                        current.clone(),
                                        full_queue,
                                    );

                                    if let Err(e) = checkpointer.save(&checkpoint).await {
                                        let graph_err = GraphError::from(e);
                                        if let Some((manager, root)) = &ctx.callbacks {
                                            let error_value = ensure_object(
                                                graph_err.to_string().to_trace_output(),
                                            );
                                            let duration_ms =
                                                root.start_instant.elapsed().as_millis();
                                            manager.on_error(root, &error_value, duration_ms).await;
                                        }
                                        ctx.pending_events.push_back(GraphEvent::Error(graph_err));
                                        ctx.join_set.shutdown().await;
                                        continue;
                                    } else {
                                        ctx.pending_events.push_back(GraphEvent::CheckpointSaved {
                                            node: current.clone(),
                                            timestamp: Utc::now().timestamp_millis() as u64,
                                        });

                                        if let Some((manager, root)) = &ctx.callbacks {
                                            // Checkpoint saved event
                                            manager
                                                .on_event(
                                                    root,
                                                    "checkpoint_saved",
                                                    &json!({"node_id": current}),
                                                )
                                                .await;
                                        }
                                    }
                                }

                                // 4b. Interrupt After (AFTER checkpoint)
                                if ctx.effective.interrupt_after.contains(&current)
                                    || self.interrupt_after.contains(&current)
                                {
                                    let error = GraphError::Interrupted;
                                    if let Some((manager, root)) = &ctx.callbacks {
                                        let error_value =
                                            ensure_object(error.to_string().to_trace_output());
                                        let duration_ms = root.start_instant.elapsed().as_millis();
                                        manager.on_error(root, &error_value, duration_ms).await;
                                    }
                                    ctx.pending_events.push_back(GraphEvent::Error(error));
                                    continue;
                                }
                            }
                            Err(e) => {
                                // Node Failure
                                let error = GraphError::NodeFailed {
                                    node: current.clone(),
                                    source: Box::new(e),
                                };
                                if let Some((manager, _root)) = &ctx.callbacks {
                                    if let Some(node_ctx) =
                                        ctx.callback_nodes.remove(&(current.clone(), path_id))
                                    {
                                        let error_value =
                                            ensure_object(error.to_string().to_trace_output());
                                        let duration_ms =
                                            node_ctx.start_instant.elapsed().as_millis();
                                        manager
                                            .on_error(&node_ctx, &error_value, duration_ms)
                                            .await;
                                    }
                                }
                                ctx.join_set.shutdown().await;
                                ctx.pending_events.push_back(GraphEvent::Error(error));
                                continue;
                            }
                        }
                    }
                } else if ctx.queue.is_empty() {
                    // Done!
                    if let Some((manager, root)) = &ctx.callbacks {
                        let outputs = ensure_object(ctx.state.to_trace_output());
                        let duration_ms = root.start_instant.elapsed().as_millis();
                        manager.on_end(root, &outputs, duration_ms).await;
                    }

                    emit_status_event(
                        &ctx.agent_event_sender,
                        &mut ctx.agent_event_step,
                        &ctx.agent_event_thread_id,
                        "completed",
                        "Graph execution completed",
                    )
                    .await;

                    return None;
                }
            }
        })
        .boxed()
    }

    pub async fn invoke_graph_with_options(
        &self,
        mut state: GraphState<S>,
        mut options: ExecutionOptions,
    ) -> Result<GraphState<S>, GraphError> {
        let checkpoint_thread_id = options.checkpoint_thread_id.clone().or_else(|| {
            self.checkpointer
                .as_ref()
                .map(|(_, thread_id)| thread_id.clone())
        });

        let agent_event_sender = options.agent_event_sender.clone();
        let _agent_event_thread_id = options
            .agent_event_thread_id
            .clone()
            .or_else(|| checkpoint_thread_id.clone())
            .unwrap_or_else(|| "graph".to_string());
        let mut agent_event_step = 0usize;

        if options.auto_resume {
            if let (Some((checkpointer, _)), Some(thread_id)) =
                (self.checkpointer.as_ref(), checkpoint_thread_id.as_deref())
            {
                match checkpointer.load(thread_id).await {
                    Ok(Some(saved)) => {
                        state = saved.state;
                        // Important: when resuming, we must respect the saved queue and step
                        if !saved.queue.is_empty() {
                            options.initial_queue = Some(saved.queue);
                            options.initial_step = Some(saved.step as usize + 1);
                        } else {
                            // If queue is empty, it means the previous run finished.
                            // We use the loaded state but allow the default (or provided) initial_queue
                            // to start a new execution path from this state.
                        }
                    }
                    Ok(None) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }

        if !self.nodes.contains_key(&self.entry) {
            let error = GraphError::MissingNode {
                node: self.entry.clone(),
            };
            emit_error_event(
                &agent_event_sender,
                &mut agent_event_step,
                error.to_string(),
                Some("graph".to_string()),
            )
            .await;
            return Err(error);
        }

        let mut stream = self.stream_invoke_with_options(state.clone(), options);

        while let Some(event) = stream.next().await {
            match event {
                Ok(GraphEvent::StateUpdate(update)) => {
                    state = state.apply_update(update);
                }
                Ok(GraphEvent::Error(e)) | Err(e) => return Err(e),
                // Other events (NodeEnter, etc.) can be ignored by invoke_graph
                // as they are handled by stream side effects (observers/callbacks).
                _ => {}
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

    pub async fn get_state(&self, thread_id: &str) -> Result<Option<GraphState<S>>, GraphError> {
        if let Some((checkpointer, _)) = &self.checkpointer {
            let checkpoint = checkpointer.load(thread_id).await?;
            Ok(checkpoint.map(|cp| cp.state))
        } else {
            Ok(None)
        }
    }

    pub async fn resume(
        &self,
        checkpoint: Checkpoint<S>,
        mut options: ExecutionOptions,
    ) -> Result<GraphState<S>, GraphError> {
        options.initial_queue = Some(checkpoint.queue);
        // Start from next logical step
        options.initial_step = Some(checkpoint.step as usize + 1);
        self.invoke_graph_with_options(checkpoint.state, options)
            .await
    }

    pub async fn update_state(
        &self,
        thread_id: &str,
        values: S,
        as_node: Option<String>,
    ) -> Result<(), GraphError> {
        if let Some((checkpointer, _)) = &self.checkpointer {
            // Load current state or default
            let (mut state, step) = if let Some(checkpoint) = checkpointer.load(thread_id).await? {
                (checkpoint.state, checkpoint.step + 1)
            } else {
                (GraphState::new(S::default()), 1)
            };

            // Apply update
            let update = StateUpdate::new(values);
            state = state.apply_update(update);

            // Save new checkpoint
            let node = as_node.unwrap_or_else(|| "user".to_string());
            let checkpoint = Checkpoint::new(thread_id.to_string(), state, step, node, vec![]);
            checkpointer.save(&checkpoint).await?;
            Ok(())
        } else {
            Err(GraphError::Checkpoint("Checkpointer not configured".into()))
        }
    }
}

#[async_trait::async_trait]
impl<S: StateSchema<Update = S>> Runnable<GraphState<S>, StateUpdate<S>> for ExecutableGraph<S> {
    async fn invoke(&self, input: GraphState<S>) -> Result<StateUpdate<S>, WesichainError> {
        let result = self
            .invoke_graph(input)
            .await
            .map_err(|e| WesichainError::Custom(e.to_string()))?;
        Ok(StateUpdate::new(result.data))
    }

    fn stream<'a>(
        &'a self,
        input: GraphState<S>,
    ) -> BoxStream<'a, Result<wesichain_core::StreamEvent, WesichainError>> {
        let stream = self.stream_invoke(input);

        stream
            .filter_map(|event_res| async move {
                match event_res {
                    Ok(GraphEvent::Error(e)) | Err(e) => {
                        Some(Err(WesichainError::Custom(e.to_string())))
                    }
                    // In a real implementation, we would map Node events to metadata
                    // or if the graph output was compatible, stream chunks.
                    // For now, subgraphs are mostly opaque unless we add a specific event mapper.
                    _ => None,
                }
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stable_path_hashing() {
        let parent_id = 12345u64;
        let node_name = "test_node";

        // Hash with our specific fixed keys
        let state = RandomState::with_seeds(0x517cc1b727220a95, 0x6ed9eba1999cd92d, 0, 0);
        let hash1 = state.hash_one((parent_id, node_name));

        // Re-compute to ensure determinism
        let expected1 = state.hash_one((parent_id, node_name));
        assert_eq!(hash1, expected1, "Hash MUST be deterministic");

        let different_hash =
            RandomState::with_seeds(123, 456, 0, 0).hash_one((parent_id, node_name));

        assert_ne!(hash1, different_hash, "Should differ from arbitrary keys");
    }
}
