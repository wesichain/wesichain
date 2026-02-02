use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::{GraphError, GraphState, StateSchema};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(bound = "S: StateSchema")]
pub struct Checkpoint<S: StateSchema> {
    pub thread_id: String,
    pub state: GraphState<S>,
}

impl<S: StateSchema> Checkpoint<S> {
    pub fn new(thread_id: String, state: GraphState<S>) -> Self {
        Self { thread_id, state }
    }
}

#[async_trait::async_trait]
pub trait Checkpointer<S: StateSchema>: Send + Sync {
    async fn save(&self, checkpoint: &Checkpoint<S>) -> Result<(), GraphError>;
    async fn load(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>, GraphError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointMetadata {
    pub seq: u64,
    pub created_at: String,
}

#[async_trait::async_trait]
pub trait HistoryCheckpointer<S: StateSchema>: Send + Sync {
    async fn list_checkpoints(&self, thread_id: &str) -> Result<Vec<CheckpointMetadata>, GraphError>;
}

#[derive(Default, Clone)]
pub struct InMemoryCheckpointer<S: StateSchema> {
    inner: Arc<RwLock<HashMap<String, Checkpoint<S>>>>,
}

#[async_trait::async_trait]
impl<S: StateSchema> Checkpointer<S> for InMemoryCheckpointer<S> {
    async fn save(&self, checkpoint: &Checkpoint<S>) -> Result<(), GraphError> {
        let mut guard = self
            .inner
            .write()
            .map_err(|_| GraphError::Checkpoint("lock".into()))?;
        guard.insert(checkpoint.thread_id.clone(), checkpoint.clone());
        Ok(())
    }

    async fn load(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>, GraphError> {
        let guard = self
            .inner
            .read()
            .map_err(|_| GraphError::Checkpoint("lock".into()))?;
        Ok(guard.get(thread_id).cloned())
    }
}
