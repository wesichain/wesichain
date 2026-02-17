mod history;
mod keys;
mod script;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use fred::interfaces::{KeysInterface, LuaInterface, SortedSetsInterface};
use fred::prelude::*;
use tokio::sync::RwLock;
use crate::keys::{index_key, safe_thread_id, ThreadKeys};
use crate::script::LUA_SAVE;
use wesichain_core::checkpoint::{Checkpoint, Checkpointer};
use wesichain_core::state::{GraphState, StateSchema};
use wesichain_core::WesichainError;

pub use keys::{index_key as redis_index_key, safe_thread_id as validate_thread_id};

#[derive(Clone)]
pub struct RedisCheckpointer {
    client: RedisClient,
    namespace: String,
    ttl_seconds: Option<u64>,
    script_sha: Arc<RwLock<String>>,
}

impl std::fmt::Debug for RedisCheckpointer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisCheckpointer")
            .field("namespace", &self.namespace)
            .field("ttl_seconds", &self.ttl_seconds)
            .finish()
    }
}

pub(crate) fn checkpoint_error(message: impl Into<String>) -> WesichainError {
    WesichainError::CheckpointFailed(message.into())
}

pub(crate) fn map_redis_error(error: RedisError) -> WesichainError {
    checkpoint_error(error.to_string())
}

impl RedisCheckpointer {
    pub async fn new(url: &str, namespace: impl Into<String>) -> Result<Self, WesichainError> {
        let config = RedisConfig::from_url(url).map_err(map_redis_error)?;
        let client = RedisClient::new(config, None, None, None);
        client.init().await.map_err(map_redis_error)?;

        let script_sha = client
            .script_load::<String, _>(LUA_SAVE)
            .await
            .map_err(map_redis_error)?;

        Ok(Self {
            client,
            namespace: namespace.into(),
            ttl_seconds: None,
            script_sha: Arc::new(RwLock::new(script_sha)),
        })
    }

    pub fn with_ttl(mut self, seconds: u64) -> Self {
        self.ttl_seconds = Some(seconds);
        self
    }

    async fn eval_save(&self, keys: Vec<String>, args: Vec<String>) -> Result<u64, WesichainError> {
        let existing_sha = self.script_sha.read().await.clone();

        match self
            .client
            .evalsha::<u64, _, _, _>(existing_sha, keys.clone(), args.clone())
            .await
        {
            Ok(seq) => Ok(seq),
            Err(error) if error.to_string().to_ascii_uppercase().contains("NOSCRIPT") => {
                let new_sha = self
                    .client
                    .script_load::<String, _>(LUA_SAVE)
                    .await
                    .map_err(map_redis_error)?;
                *self.script_sha.write().await = new_sha.clone();

                self.client
                    .evalsha::<u64, _, _, _>(new_sha, keys, args)
                    .await
                    .map_err(map_redis_error)
            }
            Err(error) => Err(map_redis_error(error)),
        }
    }
}

#[async_trait::async_trait]
impl<S> Checkpointer<S> for RedisCheckpointer
where
    S: StateSchema,
{
    async fn save(&self, checkpoint: &Checkpoint<S>) -> Result<(), WesichainError> {
        let thread_id = safe_thread_id(&checkpoint.thread_id)?;
        let keys = ThreadKeys::new(&self.namespace, thread_id);

        let payload = serde_json::to_string(checkpoint).map_err(|error| {
            checkpoint_error(format!("failed to serialize checkpoint: {error}"))
        })?;
        let ttl = self.ttl_seconds.unwrap_or(0).to_string();

        let _seq = self
            .eval_save(
                vec![keys.seq, keys.latest, keys.hist_prefix],
                vec![payload, ttl],
            )
            .await?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64;

        let _ = self
            .client
            .zadd::<(), _, _>(
                index_key(&self.namespace),
                None,
                None,
                false,
                false,
                vec![(now_ms, thread_id.to_string())],
            )
            .await;

        Ok(())
    }

    async fn load(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>, WesichainError> {
        let thread_id = safe_thread_id(thread_id)?;
        let keys = ThreadKeys::new(&self.namespace, thread_id);

        let payload: Option<String> = self
            .client
            .get(&keys.latest)
            .await
            .map_err(map_redis_error)?;

        let Some(payload) = payload else {
            return Ok(None);
        };

        let checkpoint = serde_json::from_str::<Checkpoint<S>>(&payload).map_err(|error| {
            checkpoint_error(format!("failed to deserialize checkpoint payload: {error}"))
        })?;

        Ok(Some(checkpoint))
    }
}
