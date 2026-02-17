use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use futures::stream::{self, BoxStream, StreamExt};
use petgraph::graph::Graph;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::{
    Checkpoint, Checkpointer, EdgeKind, ExecutionConfig, ExecutionOptions, GraphError, GraphEvent,
    GraphProgram, GraphState, NodeData, Observer, StateSchema, StateUpdate, END, START,
};
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
    ) -> BoxStream<'_, Result<GraphEvent, GraphError>> {
        if !self.nodes.contains_key(&self.entry) {
            return stream::iter(vec![Ok(GraphEvent::Error(GraphError::MissingNode {
                node: self.entry.clone(),
            }))])
            .boxed();
        }

        struct StreamState<S: StateSchema> {
            state: GraphState<S>,
            step_count: usize,
            recent: VecDeque<String>,
            pending_events: VecDeque<GraphEvent>,
            effective: ExecutionConfig,
            queue: VecDeque<(String, u64)>,
            join_set: JoinSet<(String, Result<StateUpdate<S>, WesichainError>, u64)>,
            start_time: std::time::Instant,
            visit_counts: HashMap<String, u32>,
            path_visits: HashMap<(String, u64), u32>,
        }

        let state = StreamState {
            state,
            step_count: 0,
            recent: VecDeque::new(),
            pending_events: VecDeque::new(),
            effective: self.default_config.merge(&ExecutionOptions::default()),
            queue: VecDeque::from([(self.entry.clone(), 0)]),
            join_set: JoinSet::new(),
            start_time: std::time::Instant::now(),
            visit_counts: HashMap::new(),
            path_visits: HashMap::new(),
        };

        stream::unfold(state, move |mut ctx| async move {
            loop {
                if let Some(event) = ctx.pending_events.pop_front() {
                    return Some((Ok(event), ctx));
                }

                if let Some((current, path_id)) = ctx.queue.pop_front() {
                    // Global Duration Check
                    if let Some(duration) = ctx.effective.max_duration {
                        if ctx.start_time.elapsed() > duration {
                            ctx.pending_events
                                .push_back(GraphEvent::Error(GraphError::Timeout {
                                    node: "global".to_string(),
                                    elapsed: ctx.start_time.elapsed(),
                                }));
                            ctx.join_set.shutdown().await;
                            continue;
                        }
                    }

                    if let Some(max) = ctx.effective.max_steps {
                        if ctx.step_count >= max {
                            ctx.pending_events.push_back(GraphEvent::Error(
                                GraphError::MaxStepsExceeded {
                                    max,
                                    reached: ctx.step_count,
                                },
                            ));
                            ctx.join_set.shutdown().await;
                            continue;
                        }
                    }

                    // Max Visits Check
                    if let Some(max_visits) = ctx.effective.max_visits {
                        let count = ctx.visit_counts.entry(current.clone()).or_insert(0);
                        *count += 1;
                        if *count > max_visits {
                            ctx.pending_events.push_back(GraphEvent::Error(
                                GraphError::MaxVisitsExceeded {
                                    node: current.clone(),
                                    max: max_visits,
                                },
                            ));
                            ctx.join_set.shutdown().await;
                            continue;
                        }
                    }

                    // Path-sensitive loop check
                    if let Some(max_loops) = ctx.effective.max_loop_iterations {
                        let key = (current.clone(), path_id);
                        let count = ctx.path_visits.entry(key).or_insert(0);
                        *count += 1;
                        if *count > max_loops {
                            ctx.pending_events.push_back(GraphEvent::Error(
                                GraphError::MaxLoopIterationsExceeded {
                                    node: current.clone(),
                                    max: max_loops,
                                    path_id,
                                },
                            ));
                            ctx.join_set.shutdown().await;
                            continue;
                        }
                    }

                    ctx.step_count += 1;

                    if ctx.effective.cycle_detection {
                        if ctx.recent.len() == ctx.effective.cycle_window {
                            ctx.recent.pop_front();
                        }
                        ctx.recent.push_back(current.clone());
                        let count = ctx.recent.iter().filter(|node| **node == current).count();
                        if count >= 2 {
                            ctx.pending_events.push_back(GraphEvent::Error(
                                GraphError::CycleDetected {
                                    node: current.clone(),
                                    recent: ctx.recent.iter().cloned().collect(),
                                },
                            ));
                            ctx.join_set.shutdown().await;
                            continue;
                        }
                    }

                    if self.interrupt_before.contains(&current) {
                        ctx.pending_events
                            .push_back(GraphEvent::Error(GraphError::Interrupted));
                        ctx.join_set.shutdown().await;
                        continue;
                    }

                    let node = match self.nodes.get(&current) {
                        Some(node) => node.clone(),
                        None => {
                            ctx.pending_events.push_back(GraphEvent::Error(
                                GraphError::InvalidEdge {
                                    node: current.clone(),
                                },
                            ));
                            continue;
                        }
                    };

                    ctx.pending_events.push_back(GraphEvent::NodeEnter {
                        node: current.clone(),
                        timestamp: Utc::now().timestamp_millis() as u64,
                    });

                    let effective_config = ctx.effective.clone();
                    let context = GraphContext {
                        remaining_steps: ctx
                            .effective
                            .max_steps
                            .map(|max| max.saturating_sub(ctx.step_count.saturating_sub(1))),
                        observer: None,
                        node_id: current.clone(),
                    };

                    let input_state = ctx.state.clone();

                    ctx.join_set.spawn(async move {
                        let future = node.invoke_with_context(input_state, &context);
                        let result = if let Some(timeout) = effective_config.node_timeout {
                            match tokio::time::timeout(timeout, future).await {
                                Ok(res) => res,
                                Err(_) => Err(WesichainError::Custom(format!(
                                    "Node {} timed out after {:?}",
                                    current, timeout
                                ))),
                            }
                        } else {
                            future.await
                        };
                        (current, result, path_id)
                    });

                    continue;
                }

                if !ctx.join_set.is_empty() {
                    if let Some(res) = ctx.join_set.join_next().await {
                        match res {
                            Ok((node_id, result, path_id)) => {
                                match result {
                                    Ok(update) => {
                                        let output_debug = serde_json::to_string(&update)
                                            .unwrap_or_else(|_| {
                                                "Error serializing update".to_string()
                                            });
                                        ctx.state = ctx.state.apply_update(update);

                                        ctx.pending_events.push_back(GraphEvent::NodeFinished {
                                            node: node_id.clone(),
                                            output: output_debug,
                                            timestamp: Utc::now().timestamp_millis() as u64,
                                        });

                                        if let Some((checkpointer, thread_id)) = &self.checkpointer
                                        {
                                            let checkpoint = Checkpoint::new(
                                                thread_id.clone(),
                                                ctx.state.clone(),
                                                ctx.step_count as u64,
                                                node_id.clone(),
                                                ctx.queue.iter().cloned().collect(),
                                            );
                                            if let Err(err) = checkpointer.save(&checkpoint).await {
                                                ctx.pending_events.push_back(GraphEvent::Error(
                                                    GraphError::from(err),
                                                ));
                                                ctx.join_set.shutdown().await;
                                                continue;
                                            }
                                            ctx.pending_events.push_back(
                                                GraphEvent::CheckpointSaved {
                                                    node: node_id.clone(),
                                                    timestamp: Utc::now().timestamp_millis() as u64,
                                                },
                                            );
                                        }

                                        ctx.pending_events.push_back(GraphEvent::NodeExit {
                                            node: node_id.clone(),
                                            timestamp: Utc::now().timestamp_millis() as u64,
                                        });

                                        if self.interrupt_after.contains(&node_id) {
                                            ctx.pending_events.push_back(GraphEvent::Error(
                                                GraphError::Interrupted,
                                            ));
                                            ctx.join_set.shutdown().await;
                                            continue;
                                        }

                                        if let Some(condition) = self.conditional.get(&node_id) {
                                            let targets = condition(&ctx.state);
                                            let next_paths: Vec<(String, u64)> = if targets.len()
                                                > 1
                                            {
                                                targets
                                                    .into_iter()
                                                    .map(|t| {
                                                        if t == END {
                                                            (t, path_id)
                                                        } else {
                                                            let mut s = DefaultHasher::new();
                                                            path_id.hash(&mut s);
                                                            t.hash(&mut s);
                                                            (t, s.finish())
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
                                                if self.nodes.contains_key(&next) {
                                                    ctx.queue.push_back((next, next_path_id));
                                                } else if let Some(edges) = self.edges.get(&next) {
                                                    // Subgraph flattening / implicit edges?
                                                    // existing code had this logic... I should preserve it but add hashing?
                                                    // Actually existing code:
                                                    /*
                                                    } else if let Some(edges) = self.edges.get(&next) {
                                                         for edge in edges {
                                                             ctx.queue.push_back(edge.clone());
                                                         }
                                                    */
                                                    // This looks like it handles "group" nodes or aliases?
                                                    // If "next" is not a node but has edges... it's a "pass-through"?
                                                    // I'll assume standard hashing logic for now.
                                                    // If it fans out here, it should hash.
                                                    for edge in edges {
                                                        // Forking from a ghost node?
                                                        // Let's use simple inheritance + mixin for now to be safe
                                                        let mut s = DefaultHasher::new();
                                                        next_path_id.hash(&mut s); // Chain the hash
                                                        edge.hash(&mut s);
                                                        ctx.queue
                                                            .push_back((edge.clone(), s.finish()));
                                                    }
                                                } else {
                                                    // Invalid node?
                                                    // Existing code pushed it anyway?
                                                    ctx.queue.push_back((next, next_path_id));
                                                }
                                            }
                                            continue;
                                        }

                                        if let Some(targets) = self.edges.get(&node_id) {
                                            let next_paths: Vec<(String, u64)> =
                                                if targets.len() > 1 {
                                                    targets
                                                        .iter()
                                                        .map(|t| {
                                                            if *t == END {
                                                                (t.clone(), path_id)
                                                            } else {
                                                                let mut s = DefaultHasher::new();
                                                                path_id.hash(&mut s);
                                                                t.hash(&mut s);
                                                                (t.clone(), s.finish())
                                                            }
                                                        })
                                                        .collect()
                                                } else {
                                                    targets
                                                        .iter()
                                                        .cloned()
                                                        .map(|t| (t, path_id))
                                                        .collect()
                                                };

                                            for (next, next_path_id) in next_paths {
                                                if next != END {
                                                    ctx.queue.push_back((next, next_path_id));
                                                }
                                            }
                                        }

                                        continue;
                                    }
                                    Err(err) => {
                                        ctx.pending_events.push_back(GraphEvent::Error(
                                            GraphError::NodeFailed {
                                                node: node_id,
                                                source: Box::new(err),
                                            },
                                        ));
                                        ctx.join_set.shutdown().await;
                                        continue;
                                    }
                                }
                            }
                            Err(e) => {
                                ctx.pending_events.push_back(GraphEvent::Error(
                                    GraphError::System(e.to_string()),
                                ));
                                ctx.join_set.shutdown().await;
                                continue;
                            }
                        }
                    }
                }

                return None;
            }
        })
        .boxed()
    }

    pub async fn invoke_graph_with_options(
        &self,
        mut state: GraphState<S>,
        options: ExecutionOptions,
    ) -> Result<GraphState<S>, GraphError> {
        let checkpoint_thread_id = options.checkpoint_thread_id.clone().or_else(|| {
            self.checkpointer
                .as_ref()
                .map(|(_, thread_id)| thread_id.clone())
        });
        let agent_event_sender = options.agent_event_sender.clone();
        let agent_event_thread_id = options
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
        let mut step_count = options.initial_step.unwrap_or(0);
        let mut recent: VecDeque<String> = VecDeque::new();
        // Queue stores (NodeId, PathId)
        let mut queue: VecDeque<(String, u64)> = options
            .initial_queue
            .clone()
            .map(VecDeque::from)
            .unwrap_or_else(|| VecDeque::from([(self.entry.clone(), 0)]));
        let mut join_set = tokio::task::JoinSet::new();
        // Track active tasks (NodeId, PathId) to persist them on interrupt
        let mut active_tasks: HashSet<(String, u64)> = HashSet::new();
        let mut callback_nodes: HashMap<(String, u64), RunContext> = HashMap::new();
        let start_time = Instant::now();
        let mut visit_counts: HashMap<String, u32> = HashMap::new();
        let mut path_visits: HashMap<(String, u64), u32> = HashMap::new();

        while !queue.is_empty() || !join_set.is_empty() {
            while let Some((current, path_id)) = queue.pop_front() {
                // Global Duration Check
                if let Some(duration) = effective.max_duration {
                    if start_time.elapsed() > duration {
                        let error = GraphError::Timeout {
                            node: "global".to_string(),
                            elapsed: start_time.elapsed(),
                        };
                        if let Some(obs) = &observer {
                            obs.on_error("global", &error).await;
                        }
                        join_set.abort_all();
                        return Err(error);
                    }
                }

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
                        join_set.abort_all();
                        return Err(error);
                    }
                }

                // Global visit count check (Strict safety net)
                if let Some(max_visits) = effective.max_visits {
                    let count = visit_counts.entry(current.clone()).or_insert(0);
                    *count += 1;
                    if *count > max_visits {
                        let error = GraphError::MaxVisitsExceeded {
                            node: current.clone(),
                            max: max_visits,
                        };
                        if let Some(obs) = &observer {
                            obs.on_error(&current, &error).await;
                        }
                        join_set.abort_all();
                        return Err(error);
                    }
                }

                // Path-sensitive loop check
                if let Some(max_loops) = effective.max_loop_iterations {
                    let key = (current.clone(), path_id);
                    let count = path_visits.entry(key).or_insert(0);
                    *count += 1;
                    if *count > max_loops {
                        let error = GraphError::MaxLoopIterationsExceeded {
                            node: current.clone(),
                            max: max_loops,
                            path_id,
                        };
                        if let Some(obs) = &observer {
                            obs.on_error(&current, &error).await;
                        }
                        join_set.abort_all();
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
                    // Basic cycle detection (window based) - might be redundant with path check but keep as option
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
                        join_set.abort_all();
                        return Err(error);
                    }
                }

                if effective.interrupt_before.contains(&current)
                    || self.interrupt_before.contains(&current)
                {
                    let error = GraphError::Interrupted;
                    if let Some(obs) = &observer {
                        obs.on_error(&current, &error).await;
                    }
                    if let Some((manager, root)) = &callbacks {
                        let error_value = ensure_object(error.to_string().to_trace_output());
                        let duration_ms = root.start_instant.elapsed().as_millis();
                        manager.on_error(root, &error_value, duration_ms).await;
                    }

                    if let (Some((checkpointer, _)), Some(thread_id)) =
                        (self.checkpointer.as_ref(), checkpoint_thread_id.as_deref())
                    {
                        // Push current back to queue (it hasn't run)
                        let mut full_queue = queue.iter().cloned().collect::<Vec<_>>();
                        full_queue.push((current.clone(), path_id));
                        // Add active tasks back (since we abort them)
                        full_queue.extend(active_tasks.iter().cloned());

                        let checkpoint = Checkpoint::new(
                            thread_id.to_string(),
                            state.clone(),
                            step_count as u64,
                            current.clone(),
                            full_queue,
                        );
                        let _ = checkpointer.save(&checkpoint).await;
                    }

                    join_set.abort_all();
                    return Err(error);
                }

                let node = match self.nodes.get(&current) {
                    Some(node) => node.clone(),
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
                        join_set.abort_all();
                        return Err(error);
                    }
                };

                if let Some(obs) = &observer {
                    let input_value =
                        serde_json::to_value(&state.data).unwrap_or(serde_json::Value::Null);
                    obs.on_node_start(&current, &input_value).await;
                }

                emit_status_event(
                    &agent_event_sender,
                    &mut agent_event_step,
                    &agent_event_thread_id,
                    "node_start",
                    format!("Starting node {current}"),
                )
                .await;

                if let Some((manager, root)) = &callbacks {
                    let node_ctx = root.child(RunType::Chain, current.clone());
                    let node_inputs = ensure_object(state.to_trace_input());
                    manager.on_start(&node_ctx, &node_inputs).await;
                    callback_nodes.insert((current.clone(), path_id), node_ctx);
                }

                // Prepare input and context
                let input_state = state.clone();
                let node_ctx_obs = observer.clone();
                let node_id = current.clone();

                let effective_config = effective.clone(); // Clone for task

                let context = GraphContext {
                    remaining_steps: effective_config
                        .max_steps
                        .map(|m| m.saturating_sub(step_count)),
                    observer: node_ctx_obs,
                    node_id: node_id.clone(),
                };

                // Track active task
                active_tasks.insert((current.clone(), path_id));

                // Spawn task with Node Timeout
                join_set.spawn(async move {
                    let future = node.invoke_with_context(input_state, &context);

                    let result = if let Some(timeout) = effective_config.node_timeout {
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
            }
            if !join_set.is_empty() {
                // Check global timeout before waiting on tasks
                if let Some(duration) = effective.max_duration {
                    if start_time.elapsed() > duration {
                        let error = GraphError::Timeout {
                            node: "global".to_string(),
                            elapsed: start_time.elapsed(),
                        };
                        if let Some(obs) = &observer {
                            obs.on_error("global", &error).await;
                        }
                        join_set.abort_all();
                        return Err(error);
                    }
                }

                if let Some(join_res) = join_set.join_next().await {
                    let (current, invoke_res, path_id) = match join_res {
                        Ok(r) => r,
                        Err(err) => {
                            let error = GraphError::System(err.to_string());
                            join_set.abort_all();
                            return Err(error);
                        }
                    };

                    // Task finished, remove from active set
                    active_tasks.remove(&(current.clone(), path_id));

                    let update = match invoke_res {
                        Ok(u) => u,
                        Err(err) => {
                            let error = GraphError::NodeFailed {
                                node: current.clone(),
                                source: Box::new(err),
                            };
                            if let Some((manager, _root)) = &callbacks {
                                if let Some(node_ctx) =
                                    callback_nodes.remove(&(current.clone(), path_id))
                                {
                                    let error_value =
                                        ensure_object(error.to_string().to_trace_output());
                                    let duration_ms = node_ctx.start_instant.elapsed().as_millis();
                                    manager.on_error(&node_ctx, &error_value, duration_ms).await;
                                }
                            }
                            if let Some(obs) = &observer {
                                obs.on_error(&current, &error).await;
                            }
                            if let Some((manager, root)) = &callbacks {
                                let error_value =
                                    ensure_object(error.to_string().to_trace_output());
                                let duration_ms = root.start_instant.elapsed().as_millis();
                                manager.on_error(root, &error_value, duration_ms).await;
                            }
                            join_set.abort_all();
                            return Err(error);
                        }
                    };

                    state = state.apply_update(update);

                    if let Some((manager, _root)) = &callbacks {
                        if let Some(node_ctx) = callback_nodes.remove(&(current.clone(), path_id)) {
                            let node_outputs = ensure_object(state.to_trace_output());
                            let duration_ms = node_ctx.start_instant.elapsed().as_millis();
                            manager.on_end(&node_ctx, &node_outputs, duration_ms).await;
                        }
                    }

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
                                    let error_value =
                                        ensure_object(error.to_string().to_trace_output());
                                    let duration_ms = root.start_instant.elapsed().as_millis();
                                    manager.on_error(root, &error_value, duration_ms).await;
                                }
                                join_set.abort_all();
                                return Err(error);
                            }
                        };
                        obs.on_node_end(&current, &output_value, 0).await;
                    }

                    emit_status_event(
                        &agent_event_sender,
                        &mut agent_event_step,
                        &agent_event_thread_id,
                        "node_end",
                        format!("Completed node {current}"),
                    )
                    .await;

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
                        join_set.abort_all();
                        return Err(error);
                    }
                    if let Some(condition) = self.conditional.get(&current) {
                        let targets = condition(&state);
                        let next_paths: Vec<(String, u64)> = if targets.len() > 1 {
                            targets
                                .into_iter()
                                .map(|t| {
                                    if t == END {
                                        (t, path_id)
                                    } else {
                                        let mut s = DefaultHasher::new();
                                        path_id.hash(&mut s);
                                        t.hash(&mut s);
                                        (t, s.finish())
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
                                let error = GraphError::InvalidEdge { node: next.clone() };
                                if let Some(obs) = &observer {
                                    obs.on_error(&current, &error).await;
                                }
                                if let Some((manager, root)) = &callbacks {
                                    let error_value =
                                        ensure_object(error.to_string().to_trace_output());
                                    let duration_ms = root.start_instant.elapsed().as_millis();
                                    manager.on_error(root, &error_value, duration_ms).await;
                                }
                                join_set.abort_all();
                                return Err(error);
                            }
                            queue.push_back((next, next_path_id));
                        }
                        continue;
                    }

                    if let Some(targets) = self.edges.get(&current) {
                        let next_paths: Vec<(String, u64)> = if targets.len() > 1 {
                            targets
                                .iter()
                                .map(|t| {
                                    if *t == END {
                                        (t.clone(), path_id)
                                    } else {
                                        let mut s = DefaultHasher::new();
                                        path_id.hash(&mut s);
                                        t.hash(&mut s);
                                        (t.clone(), s.finish())
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
                                let error = GraphError::InvalidEdge { node: next.clone() };
                                if let Some(obs) = &observer {
                                    obs.on_error(&current, &error).await;
                                }
                                if let Some((manager, root)) = &callbacks {
                                    let error_value =
                                        ensure_object(error.to_string().to_trace_output());
                                    let duration_ms = root.start_instant.elapsed().as_millis();
                                    manager.on_error(root, &error_value, duration_ms).await;
                                }
                                join_set.abort_all();
                                return Err(error);
                            }
                            queue.push_back((next, next_path_id));
                        }
                    }

                    if let (Some((checkpointer, _)), Some(thread_id)) =
                        (self.checkpointer.as_ref(), checkpoint_thread_id.as_deref())
                    {
                        let full_queue: Vec<_> = queue
                            .iter()
                            .cloned()
                            .chain(active_tasks.iter().cloned())
                            .collect();

                        let checkpoint = Checkpoint::new(
                            thread_id.to_string(),
                            state.clone(),
                            step_count as u64,
                            current.clone(),
                            full_queue,
                        );
                        if let Err(err) = checkpointer.save(&checkpoint).await {
                            let graph_err = GraphError::from(err);
                            if let Some(obs) = &observer {
                                obs.on_error(&current, &graph_err).await;
                            }
                            if let Some((manager, root)) = &callbacks {
                                let error_value =
                                    ensure_object(graph_err.to_string().to_trace_output());
                                let duration_ms = root.start_instant.elapsed().as_millis();
                                manager.on_error(root, &error_value, duration_ms).await;
                            }
                            join_set.abort_all();
                            return Err(graph_err);
                        }
                        if let Some(obs) = &observer {
                            obs.on_checkpoint_saved(&current).await;
                        }
                    }

                    if effective.interrupt_after.contains(&current)
                        || self.interrupt_after.contains(&current)
                    {
                        let error = GraphError::Interrupted;
                        if let Some(obs) = &observer {
                            obs.on_error(&current, &error).await;
                        }
                        if let Some((manager, root)) = &callbacks {
                            let error_value = ensure_object(error.to_string().to_trace_output());
                            let duration_ms = root.start_instant.elapsed().as_millis();
                            manager.on_error(root, &error_value, duration_ms).await;
                        }
                        join_set.abort_all();
                        return Err(error);
                    }
                }
            }
        }
        if let Some((manager, root)) = &callbacks {
            let outputs = ensure_object(state.to_trace_output());
            let duration_ms = root.start_instant.elapsed().as_millis();
            manager.on_end(root, &outputs, duration_ms).await;
        }

        emit_status_event(
            &agent_event_sender,
            &mut agent_event_step,
            &agent_event_thread_id,
            "completed",
            "Graph execution completed",
        )
        .await;

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
