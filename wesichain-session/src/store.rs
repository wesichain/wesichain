//! `SessionStore` trait — pluggable session persistence backend.

use async_trait::async_trait;

use crate::error::SessionError;
use crate::session::Session;

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn load(&self, id: &str) -> Result<Option<Session>, SessionError>;
    async fn save(&self, session: &Session) -> Result<(), SessionError>;
    async fn delete(&self, id: &str) -> Result<(), SessionError>;
}
