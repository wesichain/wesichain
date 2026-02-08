use std::path::Path;

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;
use wesichain_core::{AgentEvent, Document};

pub mod adapters;

#[derive(Debug, thiserror::Error)]
pub enum RagError {
    #[error("operation not implemented yet: {0}")]
    NotImplemented(&'static str),
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

#[derive(Clone, Debug)]
pub struct WesichainRag {
    event_buffer_size: usize,
}

#[derive(Clone, Debug)]
pub struct WesichainRagBuilder {
    event_buffer_size: usize,
}

impl WesichainRag {
    pub fn builder() -> WesichainRagBuilder {
        WesichainRagBuilder {
            event_buffer_size: 64,
        }
    }

    pub fn event_buffer_size(&self) -> usize {
        self.event_buffer_size
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
        let answer = format!("Stub answer for: {}", request.query);

        Ok(RagQueryResponse { answer, thread_id })
    }

    pub async fn query_stream(
        &self,
        request: RagQueryRequest,
    ) -> Result<ReceiverStream<Result<AgentEvent, RagError>>, RagError> {
        let thread_id = request
            .thread_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let query = request.query;
        let (tx, rx) = mpsc::channel(self.event_buffer_size);

        tokio::spawn(async move {
            let mut step = 0usize;

            step += 1;
            let _ = tx
                .send(Ok(AgentEvent::Status {
                    stage: "thinking".to_string(),
                    message: format!("Processing query: {query}"),
                    step,
                    thread_id: thread_id.clone(),
                }))
                .await;

            step += 1;
            let _ = tx
                .send(Ok(AgentEvent::Final {
                    content: format!("Stub answer for: {query}"),
                    step,
                }))
                .await;

            step += 1;
            let _ = tx
                .send(Ok(AgentEvent::Status {
                    stage: "completed".to_string(),
                    message: "Query finished".to_string(),
                    step,
                    thread_id,
                }))
                .await;
        });

        Ok(ReceiverStream::new(rx))
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

    pub fn with_checkpointer<T>(self, _checkpointer: T) -> Self
    where
        T: Send + Sync + 'static,
    {
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
        })
    }
}
