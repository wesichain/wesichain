//! LangSmith observability for Wesichain graphs.
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! use secrecy::SecretString;
//! use wesichain_graph::{ExecutionOptions, ExecutableGraph, GraphState, StateSchema};
//! use wesichain_langsmith::{LangSmithConfig, LangSmithObserver};
//!
//! #[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
//! struct DemoState;
//!
//! impl StateSchema for DemoState {}
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = LangSmithConfig {
//!         api_key: SecretString::new("key".to_string()),
//!         api_url: "https://api.smith.langchain.com".to_string(),
//!         project_name: "example".to_string(),
//!         flush_interval: Duration::from_secs(2),
//!         max_batch_size: 50,
//!         queue_capacity: 1000,
//!         sampling_rate: 1.0,
//!         redact_regex: None,
//!     };
//!
//!     let observer = Arc::new(LangSmithObserver::new(config));
//!     let options = ExecutionOptions {
//!         observer: Some(observer.clone()),
//!         ..Default::default()
//!     };
//!
//!     let graph: ExecutableGraph<DemoState> = todo!("build with GraphBuilder");
//!     let state = GraphState::new(DemoState::default());
//!     let _ = graph.invoke_with_options(state, options).await;
//!     let _ = observer.flush(Duration::from_secs(5)).await;
//! }
//! ```
mod client;
mod config;
mod events;
mod exporter;
mod observer;
mod run_store;
mod sampler;
mod sanitize;

pub use client::{LangSmithClient, LangSmithError};
pub use config::LangSmithConfig;
pub use events::{LangSmithInputs, LangSmithOutputs, RunEvent, RunStatus, RunType};
pub use exporter::{FlushError, FlushStats, LangSmithExporter};
pub use observer::LangSmithObserver;
pub use run_store::{RunContextStore, RunMetadata, RunUpdateDecision};
pub use sampler::{ProbabilitySampler, Sampler};
pub use sanitize::{ensure_object, sanitize_value, truncate_value};
