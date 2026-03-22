//! File-backed session store.
//!
//! Each session is stored as `<base_dir>/<session_id>.json`.
//! Writes are atomic: data is written to a `.tmp` file then renamed.

use std::path::PathBuf;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;

use crate::error::SessionError;
use crate::session::Session;
use crate::store::SessionStore;

#[derive(Clone)]
pub struct FileSessionStore {
    base_dir: PathBuf,
}

impl FileSessionStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self { base_dir: base_dir.into() }
    }

    fn session_path(&self, id: &str) -> PathBuf {
        self.base_dir.join(format!("{id}.json"))
    }

    fn tmp_path(&self, id: &str) -> PathBuf {
        self.base_dir.join(format!("{id}.json.tmp"))
    }
}

#[async_trait]
impl SessionStore for FileSessionStore {
    async fn load(&self, id: &str) -> Result<Option<Session>, SessionError> {
        let path = self.session_path(id);
        match tokio::fs::read(&path).await {
            Ok(bytes) => {
                let session: Session = serde_json::from_slice(&bytes)?;
                Ok(Some(session))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SessionError::Io(e)),
        }
    }

    async fn save(&self, session: &Session) -> Result<(), SessionError> {
        tokio::fs::create_dir_all(&self.base_dir).await?;
        let tmp = self.tmp_path(&session.id);
        let final_path = self.session_path(&session.id);
        let json = serde_json::to_vec_pretty(session)?;
        let mut f = tokio::fs::File::create(&tmp).await?;
        f.write_all(&json).await?;
        f.flush().await?;
        drop(f);
        tokio::fs::rename(&tmp, &final_path).await?;
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), SessionError> {
        let path = self.session_path(id);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(SessionError::Io(e)),
        }
    }
}
