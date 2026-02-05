//! # LangSmith Observability for Wesichain
//!
//! Enable LangSmith tracing by attaching a callback handler to your graph execution.
//!
//! ```no_run
//! use std::collections::BTreeMap;
//! use std::sync::Arc;
//!
//! use futures::StreamExt;
//! use secrecy::SecretString;
//! use wesichain_core::callbacks::{CallbackManager, RunConfig};
//! use wesichain_core::{Runnable, WesichainError};
//! use wesichain_graph::{ExecutionOptions, GraphBuilder, GraphState, StateSchema, StateUpdate};
//! use wesichain_langsmith::{LangSmithCallbackHandler, LangSmithConfig};
//!
//! #[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
//! struct DemoState {
//!     value: usize,
//! }
//!
//! impl StateSchema for DemoState {}
//!
//! struct IncrNode;
//!
//! #[async_trait::async_trait]
//! impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for IncrNode {
//!     async fn invoke(
//!         &self,
//!         input: GraphState<DemoState>,
//!     ) -> Result<StateUpdate<DemoState>, WesichainError> {
//!         Ok(StateUpdate::new(DemoState {
//!             value: input.data.value + 1,
//!         }))
//!     }
//!
//!     fn stream(
//!         &self,
//!         _input: GraphState<DemoState>,
//!     ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
//!         futures::stream::empty().boxed()
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = LangSmithConfig::new(SecretString::new("ls-api-key".to_string()), "demo");
//!     let handler = Arc::new(LangSmithCallbackHandler::new(config));
//!     let manager = CallbackManager::new(vec![handler]);
//!
//!     let options = ExecutionOptions {
//!         run_config: Some(RunConfig {
//!             callbacks: Some(manager),
//!             tags: vec!["example".to_string()],
//!             metadata: BTreeMap::new(),
//!             name_override: Some("demo-graph".to_string()),
//!         }),
//!         ..Default::default()
//!     };
//!
//!     let graph = GraphBuilder::new()
//!         .add_node("incr", IncrNode)
//!         .set_entry("incr")
//!         .build();
//!     let _ = graph
//!         .invoke_with_options(GraphState::new(DemoState::default()), options)
//!         .await?;
//!     Ok(())
//! }
//! ```
mod client;
mod config;
mod events;
mod exporter;
mod handler;
mod run_store;
mod sampler;
mod sanitize;

pub use client::{LangSmithClient, LangSmithError};
pub use config::LangSmithConfig;
pub use events::{RunEvent, RunStatus, RunType};
pub use exporter::{FlushError, FlushStats, LangSmithExporter};
pub use handler::LangSmithCallbackHandler;
pub use run_store::{RunContextStore, RunMetadata, RunUpdateDecision};
pub use sampler::{ProbabilitySampler, Sampler};
pub use sanitize::{ensure_object, sanitize_value, truncate_value};
