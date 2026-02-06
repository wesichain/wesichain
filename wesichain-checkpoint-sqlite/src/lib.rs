use std::convert::TryFrom;

use wesichain_checkpoint_sql::error::CheckpointSqlError;
use wesichain_checkpoint_sql::migrations::run_migrations;
use wesichain_checkpoint_sql::ops::{load_latest_checkpoint, save_checkpoint};
use wesichain_graph::{Checkpoint, Checkpointer, GraphError, GraphState, StateSchema};

#[derive(Debug, Clone)]
pub struct SqliteCheckpointer {
    pool: sqlx::SqlitePool,
    enable_projections: bool,
}

#[derive(Debug, Clone)]
pub struct SqliteCheckpointerBuilder {
    database_url: String,
    max_connections: u32,
    enable_projections: bool,
}

impl SqliteCheckpointer {
    pub fn builder(database_url: impl Into<String>) -> SqliteCheckpointerBuilder {
        SqliteCheckpointerBuilder {
            database_url: database_url.into(),
            max_connections: 1,
            enable_projections: false,
        }
    }

    pub fn projections_enabled(&self) -> bool {
        self.enable_projections
    }
}

impl SqliteCheckpointerBuilder {
    pub fn max_connections(mut self, max_connections: u32) -> Self {
        self.max_connections = max_connections;
        self
    }

    pub fn enable_projections(mut self, enable_projections: bool) -> Self {
        self.enable_projections = enable_projections;
        self
    }

    pub async fn build(self) -> Result<SqliteCheckpointer, CheckpointSqlError> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(self.max_connections)
            .connect(&self.database_url)
            .await
            .map_err(CheckpointSqlError::Connection)?;

        run_migrations(&pool).await?;

        Ok(SqliteCheckpointer {
            pool,
            enable_projections: self.enable_projections,
        })
    }
}

fn graph_checkpoint_error(message: impl Into<String>) -> GraphError {
    GraphError::Checkpoint(message.into())
}

fn map_sql_error(error: CheckpointSqlError) -> GraphError {
    graph_checkpoint_error(error.to_string())
}

impl<S: StateSchema> Checkpointer<S> for SqliteCheckpointer {
    fn save<'life0, 'life1, 'async_trait>(
        &'life0 self,
        checkpoint: &'life1 Checkpoint<S>,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<(), GraphError>> + Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let step = i64::try_from(checkpoint.step)
                .map_err(|_| graph_checkpoint_error("checkpoint step does not fit into i64"))?;

            save_checkpoint(
                &self.pool,
                &checkpoint.thread_id,
                &checkpoint.node,
                step,
                &checkpoint.created_at,
                &checkpoint.state,
            )
            .await
            .map_err(map_sql_error)?;

            Ok(())
        })
    }

    fn load<'life0, 'life1, 'async_trait>(
        &'life0 self,
        thread_id: &'life1 str,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<Option<Checkpoint<S>>, GraphError>> + Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let stored = load_latest_checkpoint(&self.pool, thread_id)
                .await
                .map_err(map_sql_error)?;

            let Some(stored) = stored else {
                return Ok(None);
            };

            let step_i64 = stored.step.unwrap_or_default();
            let step = u64::try_from(step_i64)
                .map_err(|_| graph_checkpoint_error("checkpoint step is negative"))?;

            let state: GraphState<S> =
                serde_json::from_value(stored.state_json).map_err(|error| {
                    graph_checkpoint_error(format!(
                        "failed to deserialize checkpoint state: {error}"
                    ))
                })?;

            Ok(Some(Checkpoint {
                thread_id: stored.thread_id,
                state,
                step,
                node: stored.node.unwrap_or_default(),
                created_at: stored.created_at,
            }))
        })
    }
}
