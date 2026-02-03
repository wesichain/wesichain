use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use tokio::sync::{Mutex, Notify};

use crate::{LangSmithClient, LangSmithConfig, LangSmithError, RunContextStore, RunEvent, RunType};

#[derive(Clone, Debug, Default)]
pub struct FlushStats {
    pub runs_flushed: usize,
    pub runs_failed: usize,
    pub batches_sent: usize,
    pub dropped_events: u64,
}

#[derive(Debug)]
pub enum FlushError {
    Timeout {
        waited: Duration,
        pending: usize,
    },
    Permanent {
        reason: String,
        batch_dropped: usize,
    },
}

#[derive(Clone)]
pub struct LangSmithExporter {
    config: LangSmithConfig,
    client: LangSmithClient,
    store: Arc<RunContextStore>,
    queue: Arc<Mutex<VecDeque<RunEvent>>>,
    notify: Arc<Notify>,
    dropped_events: Arc<AtomicU64>,
}

impl LangSmithExporter {
    pub fn new(config: LangSmithConfig, store: Arc<RunContextStore>) -> Self {
        let client = LangSmithClient::new(config.api_url.clone(), config.api_key.clone());
        let exporter = Self {
            config,
            client,
            store,
            queue: Arc::new(Mutex::new(VecDeque::new())),
            notify: Arc::new(Notify::new()),
            dropped_events: Arc::new(AtomicU64::new(0)),
        };
        exporter.spawn_flush_loop();
        exporter
    }

    pub async fn enqueue(&self, event: RunEvent) {
        let mut queue = self.queue.lock().await;
        if queue.len() >= self.config.queue_capacity {
            queue.pop_front();
            self.dropped_events.fetch_add(1, Ordering::Relaxed);
        }
        queue.push_back(event);
        if queue.len() >= self.config.max_batch_size {
            self.notify.notify_one();
        }
    }

    pub async fn flush(&self, timeout: Duration) -> Result<FlushStats, FlushError> {
        let start = Instant::now();
        let mut stats = FlushStats::default();
        loop {
            if start.elapsed() > timeout {
                let pending = self.queue.lock().await.len();
                return Err(FlushError::Timeout {
                    waited: start.elapsed(),
                    pending,
                });
            }

            let batch = self.drain_batch().await;
            if batch.is_empty() {
                stats.dropped_events = self.dropped_events();
                return Ok(stats);
            }

            match self.send_batch(&batch).await {
                Ok(batch_stats) => {
                    stats.runs_flushed += batch_stats.runs_flushed;
                    stats.runs_failed += batch_stats.runs_failed;
                    stats.batches_sent += 1;
                }
                Err(err) => {
                    return Err(FlushError::Permanent {
                        reason: err.to_string(),
                        batch_dropped: batch.len(),
                    });
                }
            }
        }
    }

    pub fn dropped_events(&self) -> u64 {
        self.dropped_events.load(Ordering::Relaxed)
    }

    pub async fn pending_len(&self) -> usize {
        self.queue.lock().await.len()
    }

    fn spawn_flush_loop(&self) {
        let exporter = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(exporter.config.flush_interval);
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let _ = exporter.flush(exporter.config.flush_interval).await;
                    }
                    _ = exporter.notify.notified() => {
                        let _ = exporter.flush(exporter.config.flush_interval).await;
                    }
                }
            }
        });
    }

    async fn drain_batch(&self) -> Vec<RunEvent> {
        let mut queue = self.queue.lock().await;
        let mut batch = Vec::new();
        for _ in 0..self.config.max_batch_size {
            if let Some(event) = queue.pop_front() {
                batch.push(event);
            } else {
                break;
            }
        }
        batch
    }

    async fn send_batch(&self, batch: &[RunEvent]) -> Result<FlushStats, LangSmithError> {
        let mut stats = FlushStats::default();
        for event in batch {
            self.send_event(event).await?;
            stats.runs_flushed += 1;
        }
        Ok(stats)
    }

    async fn send_event(&self, event: &RunEvent) -> Result<(), LangSmithError> {
        match event {
            RunEvent::Start {
                run_id,
                parent_run_id,
                name,
                run_type,
                start_time,
                inputs,
            } => {
                self.store.record_start(*run_id, *parent_run_id);
                let payload = json!({
                    "id": run_id,
                    "parent_run_id": parent_run_id,
                    "name": name,
                    "run_type": run_type_name(run_type),
                    "start_time": start_time.to_rfc3339(),
                    "inputs": inputs,
                    "session_name": self.config.project_name,
                });
                self.client.create_run(*run_id, &payload).await
            }
            RunEvent::Update {
                run_id,
                end_time,
                outputs,
                error,
                duration_ms,
            } => {
                let decision = self.store.apply_update(*run_id, error.clone());
                let mut payload = Map::new();
                if let Some(end_time) = end_time {
                    payload.insert("end_time".to_string(), Value::String(end_time.to_rfc3339()));
                }
                if let Some(outputs) = outputs {
                    payload.insert("outputs".to_string(), outputs.clone());
                }
                if let Some(error) = decision.error {
                    payload.insert("error".to_string(), Value::String(error));
                }
                if let Some(duration_ms) = duration_ms {
                    payload.insert("extra".to_string(), json!({"duration_ms": duration_ms}));
                }
                self.client
                    .update_run(*run_id, &Value::Object(payload))
                    .await
            }
        }
    }
}

fn run_type_name(run_type: &RunType) -> &'static str {
    match run_type {
        RunType::Chain => "chain",
        RunType::Tool => "tool",
    }
}
