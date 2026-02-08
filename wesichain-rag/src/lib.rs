use std::path::Path;
use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;
use wesichain_core::{AgentEvent, Document, Runnable, StreamEvent, WesichainError};
use wesichain_graph::{
    Checkpoint, Checkpointer, ExecutionOptions, GraphBuilder, GraphError, GraphState,
    InMemoryCheckpointer, StateSchema, StateUpdate,
};

pub mod adapters;

#[derive(Debug, thiserror::Error)]
pub enum RagError {
    #[error("operation not implemented yet: {0}")]
    NotImplemented(&'static str),
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error("runtime error: {0}")]
    Runtime(String),
}

#[derive(Clone, Debug)]
pub struct RagQueryRequest {
    pub query: String,
    pub thread_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RagQueryResponse {
    pub answer: String,
    pub thread_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RagSearchResult {
    pub document: Document,
    pub score: Option<f32>,
}

#[derive(Clone)]
pub struct WesichainRag {
    event_buffer_size: usize,
    checkpointer: Arc<dyn Checkpointer<RagRuntimeState>>,
}

#[derive(Clone)]
pub struct WesichainRagBuilder {
    event_buffer_size: usize,
    checkpointer: Arc<dyn Checkpointer<RagRuntimeState>>,
}

#[derive(Clone)]
struct SharedCheckpointer<S: StateSchema> {
    inner: Arc<dyn Checkpointer<S>>,
}

impl<S: StateSchema> SharedCheckpointer<S> {
    fn new(inner: Arc<dyn Checkpointer<S>>) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl<S: StateSchema> Checkpointer<S> for SharedCheckpointer<S> {
    async fn save(&self, checkpoint: &Checkpoint<S>) -> Result<(), GraphError> {
        self.inner.save(checkpoint).await
    }

    async fn load(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>, GraphError> {
        self.inner.load(thread_id).await
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RagRuntimeState {
    thread_id: String,
    current_query: String,
    turns: u64,
    last_answer: Option<String>,
}

impl StateSchema for RagRuntimeState {}

#[derive(Clone, Copy)]
struct GenerateAnswerNode;

#[async_trait::async_trait]
impl Runnable<GraphState<RagRuntimeState>, StateUpdate<RagRuntimeState>> for GenerateAnswerNode {
    async fn invoke(
        &self,
        input: GraphState<RagRuntimeState>,
    ) -> Result<StateUpdate<RagRuntimeState>, WesichainError> {
        let mut next = input.data;
        next.turns = next.turns.saturating_add(1);
        let answer = format!("Stub answer #{} for: {}", next.turns, next.current_query);
        next.last_answer = Some(answer);
        Ok(StateUpdate::new(next))
    }

    fn stream(
        &self,
        _input: GraphState<RagRuntimeState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

impl WesichainRag {
    pub fn builder() -> WesichainRagBuilder {
        WesichainRagBuilder {
            event_buffer_size: 64,
            checkpointer: Arc::new(InMemoryCheckpointer::<RagRuntimeState>::default()),
        }
    }

    pub fn event_buffer_size(&self) -> usize {
        self.event_buffer_size
    }

    async fn build_prompt(&self, query: &str) -> Result<String, RagError> {
        let _context = "Retrieved context stub";
        Ok(query.to_string())
    }

    pub async fn process_file(&self, _path: &Path) -> Result<(), RagError> {
        Err(RagError::NotImplemented("process_file"))
    }

    pub async fn add_documents(&self, _documents: Vec<Document>) -> Result<(), RagError> {
        Err(RagError::NotImplemented("add_documents"))
    }

    pub async fn similarity_search(
        &self,
        _query: &str,
        _k: usize,
    ) -> Result<Vec<RagSearchResult>, RagError> {
        Err(RagError::NotImplemented("similarity_search"))
    }

    pub async fn similarity_search_with_score(
        &self,
        _query: &str,
        _k: usize,
    ) -> Result<Vec<RagSearchResult>, RagError> {
        Err(RagError::NotImplemented("similarity_search_with_score"))
    }

    pub async fn query(&self, request: RagQueryRequest) -> Result<RagQueryResponse, RagError> {
        let thread_id = request
            .thread_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let mut stream = self
            .query_stream(RagQueryRequest {
                query: request.query,
                thread_id: Some(thread_id.clone()),
            })
            .await?;

        let mut answer = String::new();
        while let Some(event) = stream.next().await {
            match event? {
                AgentEvent::Final { content, .. } => answer = content,
                AgentEvent::Error { message, .. } => return Err(RagError::Runtime(message)),
                _ => {}
            }
        }

        Ok(RagQueryResponse { answer, thread_id })
    }

    pub async fn query_stream(
        &self,
        request: RagQueryRequest,
    ) -> Result<ReceiverStream<Result<AgentEvent, RagError>>, RagError> {
        let thread_id = request
            .thread_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let mut state = match self.checkpointer.load(&thread_id).await? {
            Some(checkpoint) => checkpoint.state.data,
            None => RagRuntimeState::default(),
        };
        state.thread_id = thread_id.clone();
        state.current_query = self.build_prompt(&request.query).await?;

        let graph = GraphBuilder::new()
            .add_node("generate", GenerateAnswerNode)
            .with_checkpointer(
                SharedCheckpointer::new(self.checkpointer.clone()),
                &thread_id,
            )
            .set_entry("generate")
            .build();

        let (graph_event_tx, mut graph_event_rx) =
            mpsc::channel::<AgentEvent>(self.event_buffer_size);
        let (output_tx, output_rx) =
            mpsc::channel::<Result<AgentEvent, RagError>>(self.event_buffer_size);
        let (result_tx, result_rx) =
            oneshot::channel::<Result<GraphState<RagRuntimeState>, GraphError>>();

        let options = ExecutionOptions {
            agent_event_sender: Some(graph_event_tx),
            agent_event_thread_id: Some(thread_id.clone()),
            ..ExecutionOptions::default()
        };

        tokio::spawn(async move {
            let result = graph
                .invoke_graph_with_options(GraphState::new(state), options)
                .await;
            let _ = result_tx.send(result);
        });

        tokio::spawn(async move {
            let mut last_step = 0usize;

            while let Some(event) = graph_event_rx.recv().await {
                if let Some(step) = event.step() {
                    last_step = step;
                }

                if output_tx.send(Ok(event)).await.is_err() {
                    return;
                }
            }

            match result_rx.await {
                Ok(Ok(final_state)) => {
                    let content = final_state.data.last_answer.unwrap_or_default();
                    let _ = output_tx
                        .send(Ok(AgentEvent::Final {
                            content,
                            step: last_step.saturating_add(1),
                        }))
                        .await;
                }
                Ok(Err(error)) => {
                    let _ = output_tx.send(Err(RagError::Graph(error))).await;
                }
                Err(_) => {
                    let _ = output_tx
                        .send(Err(RagError::Runtime(
                            "graph execution completion channel closed".to_string(),
                        )))
                        .await;
                }
            }
        });

        Ok(ReceiverStream::new(output_rx))
    }
}

impl WesichainRagBuilder {
    pub fn with_llm<T>(self, _llm: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        self
    }

    pub fn with_embedder<T>(self, _embedder: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        self
    }

    pub fn with_vector_store<T>(self, _vector_store: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        self
    }

    pub fn with_checkpointer<T>(mut self, checkpointer: T) -> Self
    where
        T: Checkpointer<RagRuntimeState> + 'static,
    {
        self.checkpointer = Arc::new(checkpointer);
        self
    }

    pub fn with_splitter<T>(self, _splitter: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        self
    }

    pub fn with_loader_registry<T>(self, _loader_registry: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        self
    }

    pub fn with_event_buffer_size(mut self, event_buffer_size: usize) -> Self {
        if event_buffer_size > 0 {
            self.event_buffer_size = event_buffer_size;
        }
        self
    }

    pub fn with_max_retries(self, _max_retries: usize) -> Self {
        self
    }

    pub fn build(self) -> Result<WesichainRag, RagError> {
        Ok(WesichainRag {
            event_buffer_size: self.event_buffer_size,
            checkpointer: self.checkpointer,
        })
    }
}
