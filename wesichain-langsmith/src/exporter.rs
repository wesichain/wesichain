use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde_json::{json, Value};
use thiserror::Error;
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

use crate::{
    LangSmithClient, LangSmithConfig, LangSmithError, RunContextStore, RunEvent, RunStatus,
    RunType,
};

#[derive(Clone, Debug, Default)]
pub struct FlushStats {
    pub runs_flushed: usize,
    pub events_flushed: usize,
    pub batches_flushed: usize,
}

#[derive(Debug, Error)]
pub enum FlushError {
    #[error("flush timed out after {0:?}")]
    Timeout(Duration),
    #[error(transparent)]
    LangSmith(#[from] LangSmithError),
}

#[derive(Clone)]
pub struct LangSmithExporter {
    inner: Arc<ExporterInner>,
}

struct ExporterInner {
    config: LangSmithConfig,
    client: LangSmithClient,
    store: Arc<RunContextStore>,
    queue: Mutex<VecDeque<RunEvent>>,
    dropped: AtomicUsize,
    notify: Notify,
    dotted_orders: DashMap<Uuid, String>,
}

impl LangSmithExporter {
    pub fn new(config: LangSmithConfig, store: Arc<RunContextStore>) -> Self {
        let client = LangSmithClient::new(config.api_url.clone(), config.api_key.clone());
        let inner = Arc::new(ExporterInner {
            config,
            client,
            store,
            queue: Mutex::new(VecDeque::new()),
            dropped: AtomicUsize::new(0),
            notify: Notify::new(),
            dotted_orders: DashMap::new(),
        });
        ExporterInner::spawn(inner.clone());
        Self { inner }
    }

    pub async fn enqueue(&self, event: RunEvent) {
        let mut queue = self.inner.queue.lock().await;
        let capacity = self.inner.config.queue_capacity;
        if capacity == 0 {
            self.inner.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        if queue.len() >= capacity {
            queue.pop_front();
            self.inner.dropped.fetch_add(1, Ordering::Relaxed);
        }
        queue.push_back(event);
        if queue.len() >= self.inner.config.max_batch_size {
            self.inner.notify.notify_one();
        }
    }

    pub fn dropped_events(&self) -> usize {
        self.inner.dropped.load(Ordering::Relaxed)
    }

    pub async fn flush(&self, timeout: Duration) -> Result<FlushStats, FlushError> {
        match tokio::time::timeout(timeout, self.inner.flush_all()).await {
            Ok(result) => Ok(result?),
            Err(_) => Err(FlushError::Timeout(timeout)),
        }
    }
}

impl ExporterInner {
    fn spawn(inner: Arc<Self>) {
        tokio::spawn(async move {
            inner.run().await;
        });
    }

    async fn run(self: Arc<Self>) {
        let mut interval = tokio::time::interval(self.config.flush_interval);
        loop {
            tokio::select! {
                _ = interval.tick() => {},
                _ = self.notify.notified() => {},
            }
            let _ = self.flush_batch(self.config.max_batch_size).await;
        }
    }

    async fn flush_all(&self) -> Result<FlushStats, LangSmithError> {
        let mut stats = FlushStats::default();
        loop {
            let batch_stats = self.flush_batch(self.config.max_batch_size).await?;
            if batch_stats.events_flushed == 0 {
                break;
            }
            stats.runs_flushed += batch_stats.runs_flushed;
            stats.events_flushed += batch_stats.events_flushed;
            stats.batches_flushed += batch_stats.batches_flushed;
        }
        Ok(stats)
    }

    async fn flush_batch(&self, max_batch_size: usize) -> Result<FlushStats, LangSmithError> {
        let batch = {
            let mut queue = self.queue.lock().await;
            let mut batch = Vec::new();
            while batch.len() < max_batch_size {
                match queue.pop_front() {
                    Some(event) => batch.push(event),
                    None => break,
                }
            }
            batch
        };

        if batch.is_empty() {
            return Ok(FlushStats::default());
        }

        let mut stats = FlushStats {
            batches_flushed: 1,
            ..Default::default()
        };
        for event in batch {
            self.send_event(event).await?;
            stats.events_flushed += 1;
            stats.runs_flushed += 1;
        }
        Ok(stats)
    }

    async fn send_event(&self, event: RunEvent) -> Result<(), LangSmithError> {
        match event {
            RunEvent::Start {
                run_id,
                parent_run_id,
                trace_id,
                name,
                run_type,
                start_time,
                inputs,
                tags,
                metadata,
                session_name,
            } => {
                self.store.record_start(run_id, parent_run_id);
                let dotted_order =
                    self.assign_dotted_order(run_id, parent_run_id, start_time);
                let payload = build_start_payload(
                    run_id,
                    parent_run_id,
                    trace_id,
                    dotted_order,
                    name,
                    run_type,
                    start_time,
                    inputs,
                    tags,
                    metadata,
                    session_name,
                );
                self.client.create_run(run_id, &payload).await
            }
            RunEvent::Update {
                run_id,
                end_time,
                outputs,
                error,
                duration_ms,
            } => {
                let decision = self.store.apply_update(run_id, error.clone());
                self.dotted_orders.remove(&run_id);
                let mut resolved_error = error;
                let mut resolved_outputs = outputs;
                if decision.status == RunStatus::Failed {
                    resolved_error = decision.error;
                    if resolved_error.is_some() {
                        resolved_outputs = None;
                    }
                }
                let payload = build_update_payload(
                    end_time,
                    resolved_outputs,
                    resolved_error,
                    duration_ms,
                );
                self.client.update_run(run_id, &payload).await
            }
        }
    }

    fn assign_dotted_order(
        &self,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        start_time: DateTime<Utc>,
    ) -> String {
        let current = format_dotted_segment(start_time, run_id);
        let dotted_order = match parent_run_id {
            Some(parent_id) => {
                let parent_order = self
                    .dotted_orders
                    .get(&parent_id)
                    .map(|value| value.clone())
                    .unwrap_or_else(|| format_dotted_segment(start_time, parent_id));
                format!("{parent_order}.{current}")
            }
            None => current,
        };
        self.dotted_orders.insert(run_id, dotted_order.clone());
        dotted_order
    }
}

fn build_start_payload(
    run_id: Uuid,
    parent_run_id: Option<Uuid>,
    trace_id: Uuid,
    dotted_order: String,
    name: String,
    run_type: RunType,
    start_time: DateTime<Utc>,
    inputs: Value,
    tags: Vec<String>,
    metadata: Value,
    session_name: String,
) -> Value {
    json!({
        "id": run_id,
        "parent_run_id": parent_run_id,
        "trace_id": trace_id,
        "dotted_order": dotted_order,
        "name": name,
        "run_type": run_type_str(&run_type),
        "start_time": start_time.to_rfc3339(),
        "inputs": inputs,
        "tags": tags,
        "metadata": metadata,
        "session_name": session_name,
    })
}

fn build_update_payload(
    end_time: Option<DateTime<Utc>>,
    outputs: Option<Value>,
    error: Option<String>,
    duration_ms: Option<u128>,
) -> Value {
    let mut payload = serde_json::Map::new();
    if let Some(end_time) = end_time {
        payload.insert("end_time".to_string(), Value::String(end_time.to_rfc3339()));
    }
    if let Some(outputs) = outputs {
        payload.insert("outputs".to_string(), outputs);
    }
    if let Some(error) = error {
        payload.insert("error".to_string(), Value::String(error));
    }
    if let Some(duration_ms) = duration_ms {
        payload.insert(
            "extra".to_string(),
            json!({"duration_ms": duration_to_u64(duration_ms)}),
        );
    }
    Value::Object(payload)
}

fn duration_to_u64(duration_ms: u128) -> u64 {
    u64::try_from(duration_ms).unwrap_or(u64::MAX)
}

fn run_type_str(run_type: &RunType) -> &'static str {
    match run_type {
        RunType::Chain => "chain",
        RunType::Tool => "tool",
        RunType::Llm => "llm",
        RunType::Agent => "chain",
        RunType::Graph => "chain",
    }
}

fn format_dotted_segment(start_time: DateTime<Utc>, run_id: Uuid) -> String {
    let prefix = start_time.format("%Y%m%dT%H%M%S").to_string();
    let micros = start_time.timestamp_subsec_micros();
    format!("{prefix}{micros:06}Z{run_id}")
}
