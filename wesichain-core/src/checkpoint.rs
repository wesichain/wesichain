use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::state::{GraphState, StateSchema};
use crate::WesichainError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(bound = "S: StateSchema")]
pub struct Checkpoint<S: StateSchema> {
    pub thread_id: String,
    pub state: GraphState<S>,
    pub step: u64,
    pub node: String,
    pub queue: Vec<(String, u64)>,
    pub created_at: String,
}

impl<S: StateSchema> Checkpoint<S> {
    pub fn new(
        thread_id: String,
        state: GraphState<S>,
        step: u64,
        node: String,
        queue: Vec<(String, u64)>,
    ) -> Self {
        Self {
            thread_id,
            state,
            step,
            node,
            queue,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}

#[async_trait::async_trait]
pub trait Checkpointer<S: StateSchema>: Send + Sync {
    async fn save(&self, checkpoint: &Checkpoint<S>) -> Result<(), WesichainError>;
    async fn load(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>, WesichainError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointMetadata {
    pub seq: u64,
    pub created_at: String,
}

#[async_trait::async_trait]
pub trait HistoryCheckpointer<S: StateSchema>: Send + Sync {
    async fn list_checkpoints(
        &self,
        thread_id: &str,
    ) -> Result<Vec<CheckpointMetadata>, WesichainError>;
}

#[derive(Default, Clone)]
pub struct InMemoryCheckpointer<S: StateSchema> {
    inner: Arc<RwLock<HashMap<String, Vec<Checkpoint<S>>>>>,
}

#[async_trait::async_trait]
impl<S: StateSchema> Checkpointer<S> for InMemoryCheckpointer<S> {
    async fn save(&self, checkpoint: &Checkpoint<S>) -> Result<(), WesichainError> {
        let mut guard = self
            .inner
            .write()
            .map_err(|_| WesichainError::CheckpointFailed("lock".into()))?;
        guard
            .entry(checkpoint.thread_id.clone())
            .or_default()
            .push(checkpoint.clone());
        Ok(())
    }

    async fn load(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>, WesichainError> {
        let guard = self
            .inner
            .read()
            .map_err(|_| WesichainError::CheckpointFailed("lock".into()))?;
        Ok(guard
            .get(thread_id)
            .and_then(|history| history.last().cloned()))
    }
}
#[async_trait::async_trait]
impl<S: StateSchema> HistoryCheckpointer<S> for InMemoryCheckpointer<S> {
    async fn list_checkpoints(
        &self,
        thread_id: &str,
    ) -> Result<Vec<CheckpointMetadata>, WesichainError> {
        let guard = self
            .inner
            .read()
            .map_err(|_| WesichainError::CheckpointFailed("lock".into()))?;
        let history = guard.get(thread_id).cloned().unwrap_or_default();
        let metadata = history
            .into_iter()
            .map(|cp| CheckpointMetadata {
                seq: cp.step,
                created_at: cp.created_at,
            })
            .collect();
        Ok(metadata)
    }
}
