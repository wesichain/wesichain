//! Langfuse observability integration for wesichain.
//!
//! Langfuse is an open-source LLM observability platform.  This crate provides:
//!
//! - [`LangfuseCallbackHandler`] — implements [`CallbackHandler`] and ships
//!   traces, spans, and LLM generation events to a Langfuse server.
//! - [`LangfuseClient`] — low-level HTTP client for the Langfuse ingestion API.
//! - [`LangfuseConfig`] — connection settings.
//!
//! # Quick start
//! ```no_run
//! use wesichain_langfuse::{LangfuseCallbackHandler, LangfuseConfig};
//! use wesichain_core::CallbackManager;
//!
//! let handler = LangfuseCallbackHandler::new(LangfuseConfig {
//!     public_key: "pk-lf-...".into(),
//!     secret_key: "sk-lf-...".into(),
//!     host: "https://cloud.langfuse.com".into(),
//!     ..Default::default()
//! });
//! let _manager = CallbackManager::new(vec![std::sync::Arc::new(handler)]);
//! ```

pub mod client;
pub mod config;
pub mod handler;
pub mod types;

pub use client::LangfuseClient;
pub use config::LangfuseConfig;
pub use handler::LangfuseCallbackHandler;
pub use types::{LangfuseEvent, LangfuseGeneration, LangfuseSpan, LangfuseTrace};
