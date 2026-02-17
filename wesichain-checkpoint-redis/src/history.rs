use fred::interfaces::KeysInterface;
use wesichain_core::checkpoint::{
    Checkpoint, CheckpointMetadata, HistoryCheckpointer,
};
use wesichain_core::state::StateSchema;
use wesichain_core::WesichainError;

use crate::keys::{safe_thread_id, ThreadKeys};
use crate::{checkpoint_error, map_redis_error, RedisCheckpointer};

#[async_trait::async_trait]
impl<S> HistoryCheckpointer<S> for RedisCheckpointer
where
    S: StateSchema,
{
    async fn list_checkpoints(
        &self,
        thread_id: &str,
    ) -> Result<Vec<CheckpointMetadata>, WesichainError> {
        let thread_id = safe_thread_id(thread_id)?;
        let keys = ThreadKeys::new(&self.namespace, thread_id);

        let max_seq: Option<i64> = self.client.get(&keys.seq).await.map_err(map_redis_error)?;
        let Some(max_seq) = max_seq else {
            return Ok(Vec::new());
        };

        if max_seq <= 0 {
            return Ok(Vec::new());
        }

        let mut out = Vec::new();
        for seq in 1..=max_seq {
            let history_key = format!("{}:{seq}", keys.hist_prefix);
            let payload: Option<String> = self
                .client
                .get(history_key)
                .await
                .map_err(map_redis_error)?;

            let Some(payload) = payload else {
                continue;
            };

            let checkpoint: Checkpoint<S> = serde_json::from_str(&payload).map_err(|error| {
                checkpoint_error(format!("failed to deserialize history checkpoint: {error}"))
            })?;

            out.push(CheckpointMetadata {
                seq: seq as u64,
                created_at: checkpoint.created_at,
            });
        }

        Ok(out)
    }
}
