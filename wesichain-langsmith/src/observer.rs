use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use regex::Regex;
use serde_json::Value;
use uuid::Uuid;
use wesichain_graph::{GraphError, Observer};

use crate::{
    ensure_object, sanitize_value, truncate_value, LangSmithConfig, LangSmithExporter,
    ProbabilitySampler, RunEvent, RunType, Sampler,
};

const MAX_FIELD_BYTES: usize = 100_000;

/// Observer implementation that emits LangSmith run events.
#[derive(Clone)]
pub struct LangSmithObserver {
    exporter: LangSmithExporter,
    sampler: Arc<dyn Sampler>,
    redact_regex: Option<Regex>,
    node_runs: DashMap<String, NodeRunContext>,
    tool_runs: DashMap<String, VecDeque<Uuid>>,
}

#[derive(Clone, Debug)]
struct NodeRunContext {
    run_id: Uuid,
    sampled: bool,
}

impl LangSmithObserver {
    /// Create a new observer with default sampling behavior.
    ///
    /// ```rust,no_run
    /// use secrecy::SecretString;
    /// use wesichain_langsmith::{LangSmithConfig, LangSmithObserver};
    ///
    /// let config = LangSmithConfig::new(SecretString::new("key".to_string()), "project");
    /// let observer = LangSmithObserver::new(config);
    /// let _ = observer;
    /// ```
    pub fn new(config: LangSmithConfig) -> Self {
        let sampler: Arc<dyn Sampler> = Arc::new(ProbabilitySampler {
            rate: config.sampling_rate,
        });
        Self::with_sampler(config, sampler)
    }

    pub fn with_sampler(config: LangSmithConfig, sampler: Arc<dyn Sampler>) -> Self {
        let exporter = LangSmithExporter::new(config.clone(), Arc::new(Default::default()));
        Self {
            exporter,
            sampler,
            redact_regex: config.redact_regex.clone(),
            node_runs: DashMap::new(),
            tool_runs: DashMap::new(),
        }
    }

    pub fn dropped_events(&self) -> u64 {
        self.exporter.dropped_events()
    }

    /// Flush pending events with a timeout.
    ///
    /// The timeout bounds how long to wait for the queue to drain.
    ///
    /// ```rust,no_run
    /// use std::time::Duration;
    /// use secrecy::SecretString;
    /// use wesichain_langsmith::{LangSmithConfig, LangSmithObserver};
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let config = LangSmithConfig::new(SecretString::new("key".to_string()), "project");
    /// let observer = LangSmithObserver::new(config);
    /// let _ = observer.flush(Duration::from_secs(5)).await;
    /// # }
    /// ```
    pub async fn flush(
        &self,
        timeout: std::time::Duration,
    ) -> Result<crate::FlushStats, crate::FlushError> {
        self.exporter.flush(timeout).await
    }

    fn prepare_value(&self, value: &Value) -> Value {
        let redacted = sanitize_value(value.clone(), self.redact_regex.as_ref());
        let truncated = truncate_value(redacted, MAX_FIELD_BYTES);
        ensure_object(truncated)
    }

    fn record_node_run(&self, node_id: &str) -> NodeRunContext {
        let run_id = Uuid::new_v4();
        let sampled = self.sampler.should_sample(run_id);
        let context = NodeRunContext { run_id, sampled };
        self.node_runs.insert(node_id.to_string(), context.clone());
        context
    }

    fn push_tool_run(&self, key: String, run_id: Uuid) {
        let mut entry = self.tool_runs.entry(key).or_default();
        entry.push_back(run_id);
    }

    fn pop_tool_run(&self, key: &str) -> Option<Uuid> {
        self.tool_runs
            .get_mut(key)
            .and_then(|mut entry| entry.pop_front())
    }
}

#[async_trait]
impl Observer for LangSmithObserver {
    async fn on_node_start(&self, node_id: &str, input: &Value) {
        let context = self.record_node_run(node_id);
        if !context.sampled {
            return;
        }
        let inputs = self.prepare_value(input);
        self.exporter
            .enqueue(RunEvent::Start {
                run_id: context.run_id,
                parent_run_id: None,
                name: node_id.to_string(),
                run_type: RunType::Chain,
                start_time: Utc::now(),
                inputs,
            })
            .await;
    }

    async fn on_node_end(&self, node_id: &str, output: &Value, duration_ms: u128) {
        let context = match self.node_runs.get(node_id) {
            Some(entry) => entry.clone(),
            None => return,
        };
        if !context.sampled {
            self.node_runs.remove(node_id);
            return;
        }
        let outputs = self.prepare_value(output);
        self.exporter
            .enqueue(RunEvent::Update {
                run_id: context.run_id,
                end_time: Some(Utc::now()),
                outputs: Some(outputs),
                error: None,
                duration_ms: Some(duration_ms),
            })
            .await;
        self.node_runs.remove(node_id);
    }

    async fn on_error(&self, node_id: &str, error: &GraphError) {
        let context = self
            .node_runs
            .get(node_id)
            .map(|entry| entry.clone())
            .unwrap_or_else(|| self.record_node_run(node_id));

        if !context.sampled {
            self.node_runs.remove(node_id);
            return;
        }

        self.exporter
            .enqueue(RunEvent::Update {
                run_id: context.run_id,
                end_time: Some(Utc::now()),
                outputs: None,
                error: Some(error.to_string()),
                duration_ms: None,
            })
            .await;
        self.node_runs.remove(node_id);
    }

    async fn on_tool_call(&self, node_id: &str, tool_name: &str, args: &Value) {
        let context = match self.node_runs.get(node_id) {
            Some(entry) => entry.clone(),
            None => return,
        };
        if !context.sampled {
            return;
        }
        let run_id = Uuid::new_v4();
        let key = format!("{}::{}", node_id, tool_name);
        self.push_tool_run(key, run_id);

        let inputs = self.prepare_value(args);
        self.exporter
            .enqueue(RunEvent::Start {
                run_id,
                parent_run_id: Some(context.run_id),
                name: tool_name.to_string(),
                run_type: RunType::Tool,
                start_time: Utc::now(),
                inputs,
            })
            .await;
    }

    async fn on_tool_result(&self, node_id: &str, tool_name: &str, result: &Value) {
        let context = match self.node_runs.get(node_id) {
            Some(entry) => entry.clone(),
            None => return,
        };
        if !context.sampled {
            return;
        }
        let key = format!("{}::{}", node_id, tool_name);
        let run_id = match self.pop_tool_run(&key) {
            Some(id) => id,
            None => return,
        };
        let outputs = self.prepare_value(result);
        self.exporter
            .enqueue(RunEvent::Update {
                run_id,
                end_time: Some(Utc::now()),
                outputs: Some(outputs),
                error: None,
                duration_ms: None,
            })
            .await;
    }
}
