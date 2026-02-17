use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Checkpoint, CheckpointMetadata, Checkpointer, HistoryCheckpointer, StateSchema};
use wesichain_core::WesichainError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "S: StateSchema")]
pub struct CheckpointRecord<S: StateSchema> {
    pub seq: u64,
    pub created_at: String,
    pub checkpoint: Checkpoint<S>,
}

#[derive(Clone, Debug)]
pub struct FileCheckpointer {
    base_dir: PathBuf,
}

impl FileCheckpointer {
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    fn sanitize_thread_id(thread_id: &str) -> String {
        let mut out = String::with_capacity(thread_id.len());
        for ch in thread_id.chars() {
            match ch {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => out.push('_'),
                c if c.is_control() => {}
                c => out.push(c),
            }
        }
        let trimmed = out.trim_matches(|c: char| c == '.' || c.is_whitespace() || c == '_');
        if trimmed.is_empty() {
            let mut hasher = DefaultHasher::new();
            thread_id.hash(&mut hasher);
            return format!("thread-{:08x}", hasher.finish());
        }
        trimmed.to_string()
    }

    fn thread_path(&self, thread_id: &str) -> PathBuf {
        let filename = format!("{}.jsonl", Self::sanitize_thread_id(thread_id));
        self.base_dir.join(filename)
    }

    fn next_seq<S: StateSchema>(&self, thread_id: &str) -> Result<u64, WesichainError> {
        let path = self.thread_path(thread_id);
        if !path.exists() {
            return Ok(1);
        }
        let file =
            File::open(&path).map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?;
        let reader = BufReader::new(file);
        let mut last: Option<CheckpointRecord<S>> = None;
        for line in reader.lines() {
            let line = line.map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            last = Some(
                serde_json::from_str(&line)
                    .map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?,
            );
        }
        Ok(last.map(|record| record.seq + 1).unwrap_or(1))
    }
}

#[async_trait::async_trait]
impl<S: StateSchema> Checkpointer<S> for FileCheckpointer {
    async fn save(&self, checkpoint: &Checkpoint<S>) -> Result<(), WesichainError> {
        fs::create_dir_all(&self.base_dir)
            .map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?;

        let path = self.thread_path(&checkpoint.thread_id);
        let seq = self.next_seq::<S>(&checkpoint.thread_id)?;
        let record = CheckpointRecord {
            seq,
            created_at: checkpoint.created_at.clone(),
            checkpoint: checkpoint.clone(),
        };
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?;
        let line = serde_json::to_string(&record)
            .map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?;
        file.write_all(format!("{line}\n").as_bytes())
            .map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?;
        Ok(())
    }

    async fn load(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>, WesichainError> {
        let path = self.thread_path(thread_id);
        if !path.exists() {
            return Ok(None);
        }
        let file =
            File::open(&path).map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?;
        let reader = BufReader::new(file);
        let mut last: Option<CheckpointRecord<S>> = None;
        for line in reader.lines() {
            let line = line.map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            last = Some(
                serde_json::from_str(&line)
                    .map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?,
            );
        }
        Ok(last.map(|record| record.checkpoint))
    }
}

#[async_trait::async_trait]
impl<S: StateSchema> HistoryCheckpointer<S> for FileCheckpointer {
    async fn list_checkpoints(
        &self,
        thread_id: &str,
    ) -> Result<Vec<CheckpointMetadata>, WesichainError> {
        let path = self.thread_path(thread_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file =
            File::open(&path).map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?;
        let reader = BufReader::new(file);
        let mut history = Vec::new();
        for line in reader.lines() {
            let line = line.map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let record: CheckpointRecord<S> = serde_json::from_str(&line)
                .map_err(|err| WesichainError::CheckpointFailed(err.to_string()))?;
            history.push(CheckpointMetadata {
                seq: record.seq,
                created_at: record.created_at,
            });
        }
        Ok(history)
    }
}
