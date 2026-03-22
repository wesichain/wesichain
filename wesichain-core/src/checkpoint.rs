use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

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

    /// Fork execution from a historical checkpoint.
    ///
    /// Copies all state up to and including `at_seq` from `thread_id` into a
    /// new thread and returns the new thread id.  The caller can then resume
    /// from the forked thread, creating a separate branch of execution.
    async fn fork(
        &self,
        _thread_id: &str,
        _at_seq: u64,
    ) -> Result<String, WesichainError> {
        Err(WesichainError::CheckpointFailed(
            "fork() not implemented for this checkpointer".into(),
        ))
    }
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

    async fn fork(&self, thread_id: &str, at_seq: u64) -> Result<String, WesichainError> {
        let history = {
            let guard = self
                .inner
                .read()
                .map_err(|_| WesichainError::CheckpointFailed("lock".into()))?;
            guard.get(thread_id).cloned().unwrap_or_default()
        };

        // Require that the requested seq exists before forking
        if !history.iter().any(|cp| cp.step == at_seq) {
            return Err(WesichainError::CheckpointFailed(format!(
                "no checkpoint at seq {at_seq} in thread '{thread_id}'"
            )));
        }

        // Collect all checkpoints up to and including at_seq
        let prefix: Vec<Checkpoint<S>> = history
            .into_iter()
            .filter(|cp| cp.step <= at_seq)
            .collect();

        let new_thread_id = Uuid::new_v4().to_string();

        let mut guard = self
            .inner
            .write()
            .map_err(|_| WesichainError::CheckpointFailed("lock".into()))?;

        // Re-stamp cloned checkpoints with the new thread id
        let forked: Vec<Checkpoint<S>> = prefix
            .into_iter()
            .map(|mut cp| {
                cp.thread_id = new_thread_id.clone();
                cp
            })
            .collect();

        guard.insert(new_thread_id.clone(), forked);
        Ok(new_thread_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{GraphState, StateSchema};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
    struct Counter {
        n: u32,
    }
    impl StateSchema for Counter {
        type Update = u32;
        fn apply(current: &Self, update: u32) -> Self {
            Self { n: current.n + update }
        }
    }

    fn make_cp(thread_id: &str, step: u64) -> Checkpoint<Counter> {
        Checkpoint::new(
            thread_id.to_string(),
            GraphState { data: Counter { n: step as u32 } },
            step,
            "node".to_string(),
            vec![],
        )
    }

    #[tokio::test]
    async fn fork_creates_new_thread_up_to_seq() {
        let cp: InMemoryCheckpointer<Counter> = InMemoryCheckpointer::default();
        for step in 0..5u64 {
            cp.save(&make_cp("main", step)).await.unwrap();
        }

        let fork_id = cp.fork("main", 2).await.unwrap();
        assert_ne!(fork_id, "main");

        let meta = cp.list_checkpoints(&fork_id).await.unwrap();
        assert_eq!(meta.len(), 3); // steps 0, 1, 2

        let latest = cp.load(&fork_id).await.unwrap().unwrap();
        assert_eq!(latest.step, 2);
    }

    #[tokio::test]
    async fn fork_missing_seq_errors() {
        let cp: InMemoryCheckpointer<Counter> = InMemoryCheckpointer::default();
        cp.save(&make_cp("main", 0)).await.unwrap();
        assert!(cp.fork("main", 99).await.is_err());
    }

    #[tokio::test]
    async fn fork_independent_of_origin() {
        let cp: InMemoryCheckpointer<Counter> = InMemoryCheckpointer::default();
        for step in 0..3u64 {
            cp.save(&make_cp("main", step)).await.unwrap();
        }
        let fork_id = cp.fork("main", 1).await.unwrap();

        // Saving more into main should not affect the fork
        cp.save(&make_cp("main", 3)).await.unwrap();
        let fork_meta = cp.list_checkpoints(&fork_id).await.unwrap();
        assert_eq!(fork_meta.len(), 2); // still only steps 0 and 1
    }
}
