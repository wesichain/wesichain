//! In-memory session store (not persistent across restarts).

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::error::SessionError;
use crate::session::Session;
use crate::store::SessionStore;

#[derive(Clone, Default)]
pub struct InMemorySessionStore {
    map: Arc<Mutex<HashMap<String, Session>>>,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn load(&self, id: &str) -> Result<Option<Session>, SessionError> {
        Ok(self.map.lock().await.get(id).cloned())
    }

    async fn save(&self, session: &Session) -> Result<(), SessionError> {
        self.map.lock().await.insert(session.id.clone(), session.clone());
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), SessionError> {
        self.map.lock().await.remove(id);
        Ok(())
    }
}
