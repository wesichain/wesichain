//! `SessionManager` — high-level API for session lifecycle.

use wesichain_core::Message;

use crate::error::SessionError;
use crate::session::{Session, ToolCallRecord};
use crate::store::SessionStore;

/// High-level session management: create, load, update, and compress sessions.
pub struct SessionManager<S: SessionStore> {
    store: S,
    /// Auto-compress when total chars exceeds this threshold.
    auto_compress_at: Option<usize>,
}

impl<S: SessionStore> SessionManager<S> {
    pub fn new(store: S) -> Self {
        Self { store, auto_compress_at: None }
    }

    /// Set the character threshold above which `build_context` trims old messages.
    pub fn with_auto_compress(mut self, max_chars: usize) -> Self {
        self.auto_compress_at = Some(max_chars);
        self
    }

    /// Load an existing session by `id`, or create a new one if `id` is `None`
    /// or not found.
    pub async fn get_or_create(&self, id: Option<&str>) -> Result<Session, SessionError> {
        if let Some(id) = id {
            if let Some(session) = self.store.load(id).await? {
                return Ok(session);
            }
        }
        let new_id = id.map(String::from).unwrap_or_else(|| {
            uuid::Uuid::new_v4().to_string()
        });
        let session = Session::new(new_id);
        self.store.save(&session).await?;
        Ok(session)
    }

    /// Append a completed conversation turn to the session.
    pub async fn append_turn(
        &self,
        id: &str,
        user: Message,
        assistant: Message,
        tools: Vec<ToolCallRecord>,
    ) -> Result<(), SessionError> {
        let mut session = self.store.load(id).await?.ok_or_else(|| {
            SessionError::NotFound(id.to_string())
        })?;
        session.messages.push(user);
        session.messages.push(assistant);
        session.tool_history.extend(tools);
        self.store.save(&session).await
    }

    /// Return the message history, trimming the oldest messages if the total
    /// character count exceeds `auto_compress_at`.
    pub async fn build_context(&self, id: &str) -> Result<Vec<Message>, SessionError> {
        let session = self.store.load(id).await?.ok_or_else(|| {
            SessionError::NotFound(id.to_string())
        })?;

        if let Some(max) = self.auto_compress_at {
            let mut messages = session.messages.clone();
            while messages.len() > 2 {
                let total: usize = messages.iter().map(|m| m.content.to_string().len()).sum();
                if total <= max {
                    break;
                }
                // Drop the oldest non-system message
                let drop_idx = messages.iter().position(|m| {
                    !matches!(m.role, wesichain_core::Role::System)
                }).unwrap_or(0);
                messages.remove(drop_idx);
            }
            return Ok(messages);
        }

        Ok(session.messages)
    }

    pub fn store(&self) -> &S {
        &self.store
    }
}
