//! Pinecone vector store integration for Wesichain.
//!
//! This crate provides a `PineconeVectorStore` with:
//! - external embedding provider injection (`E: Embedding`),
//! - LangChain-style methods (`add_documents`, `similarity_search`, etc.),
//! - Rust-idiomatic trait integration (`VectorStore`).
//!
//! Environment variables commonly used in examples:
//! - `PINECONE_API_KEY`
//! - `PINECONE_BASE_URL`
//! - `PINECONE_NAMESPACE` (optional)

pub mod client;
mod config;
mod error;
pub mod filter;
pub mod mapper;
mod store;
mod types;

pub use config::PineconeStoreBuilder;
pub use error::PineconeStoreError;
pub use store::PineconeVectorStore;
