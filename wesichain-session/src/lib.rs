//! Session persistence for wesichain agents.
//!
//! Provides `Session`, `SessionManager`, `InMemorySessionStore`, and `FileSessionStore`
//! to persist conversation history across invocations.
//!
//! # Quick start
//! ```ignore
//! use wesichain_session::{SessionManager, FileSessionStore};
//!
//! let manager = SessionManager::new(FileSessionStore::new("./sessions"));
//! let session = manager.get_or_create(Some("user-123")).await?;
//! let messages = manager.build_context(&session.id).await?;
//! ```

pub mod cost;
pub mod error;
pub mod manager;
pub mod session;
pub mod store;
pub mod stores;

pub use cost::{cost_for_response, price_for_model, SessionCostSummary};
pub use error::SessionError;
pub use manager::SessionManager;
pub use session::{Session, ToolCallRecord};
pub use store::SessionStore;
pub use stores::{FileSessionStore, InMemorySessionStore};
