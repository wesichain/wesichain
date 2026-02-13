//! Wesichain Compatibility Layer
//!
//! This crate provides a compatibility layer for migrating from LangChain 0.3.0 to Wesichain.
//! It exports Wesichain components with LangChain-compatible names and interfaces where possible.

pub use wesichain_core as core;

// Re-export core traits with LangChain-like names
pub use wesichain_core::Runnable as LangChainRunnable;
pub use wesichain_core::RunnableExt;

// Common types
pub use wesichain_core::StreamEvent;
pub use wesichain_core::Value;
pub use wesichain_core::WesichainError as LangChainError;
pub use wesichain_core::{Bindable, RunnableBinding};
