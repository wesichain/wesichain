# Wesichain LangSmith Observability Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add LangSmith-compatible observability for Phase 5 graphs via a non-blocking Observer implementation.

**Architecture:** Add a new `wesichain-langsmith` crate that implements the stable `wesichain-graph::Observer` trait (node/tool callbacks with JSON payloads). The observer maps events into `RunEvent`s, queues them in a bounded buffer with drop-oldest backpressure, and a background task flushes batches through a small retrying HTTP client. A run-context store enforces terminal semantics so errors are never cleared by later updates.

**Tech Stack:** Rust 1.75, tokio, async-trait, reqwest, serde/serde_json, uuid, dashmap, tracing, regex, secrecy, wiremock, chrono, thiserror.

---

### Task 0: Worktree and Phase 5 API alignment

**Files:**
- Read: `wesichain-graph/src/observer.rs`
- Read: `wesichain-graph/src/config.rs`
- Read: `wesichain-graph/src/react_agent.rs`
- Read: `wesichain-graph/tests/react_agent.rs`
- Read: `wesichain-core/src/react.rs`

**Step 1: Create a fresh worktree from main (@superpowers:using-git-worktrees)**

Run: `git fetch origin`
Run: `git worktree add .worktrees/langsmith-observability origin/main`
Run: `cd .worktrees/langsmith-observability`

Expected: Worktree created at `.worktrees/langsmith-observability`

**Step 2: Verify the Observer trait signature**

Expected in `wesichain-graph/src/observer.rs`:

```rust
#[async_trait::async_trait]
pub trait Observer: Send + Sync + 'static {
    async fn on_node_start(&self, node_id: &str, input: &serde_json::Value);
    async fn on_node_end(&self, node_id: &str, output: &serde_json::Value, duration_ms: u128);
    async fn on_error(&self, node_id: &str, error: &crate::GraphError);
    async fn on_tool_call(&self, _node_id: &str, _tool_name: &str, _args: &serde_json::Value) {}
    async fn on_tool_result(&self, _node_id: &str, _tool_name: &str, _result: &serde_json::Value) {}
}
```

**Step 3: Verify ExecutionOptions carries observer**

Expected in `wesichain-graph/src/config.rs`:

```rust
pub struct ExecutionOptions {
    pub max_steps: Option<usize>,
    pub cycle_detection: Option<bool>,
    pub cycle_window: Option<usize>,
    pub observer: Option<Arc<dyn Observer>>,
}
```

**Step 4: Confirm ReActAgentNode emits tool callbacks**

Expected in `wesichain-graph/src/react_agent.rs`:

```rust
observer.on_tool_call(&context.node_id, &call.name, &call.args).await;
observer.on_tool_result(&context.node_id, &call.name, &result).await;
```

**Step 5: If any signatures or locations differ, stop and update**

Update `docs/plans/2026-02-03-wesichain-langsmith-observer-design.md`, then update this plan before coding.

---

### Task 1: Create wesichain-langsmith crate skeleton

**Files:**
- Modify: `Cargo.toml`
- Create: `wesichain-langsmith/Cargo.toml`
- Create: `wesichain-langsmith/src/lib.rs`

**Step 1: Add workspace member**

Update `Cargo.toml`:

```toml
[workspace]
members = [
  "wesichain",
  "wesichain-core",
  "wesichain-prompt",
  "wesichain-llm",
  "wesichain-agent",
  "wesichain-graph",
  "wesichain-langsmith",
]
```

**Step 2: Create crate manifest**

Create `wesichain-langsmith/Cargo.toml`:

```toml
[package]
name = "wesichain-langsmith"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
repository.workspace = true
homepage.workspace = true
description = "LangSmith observability for Wesichain graphs"

[dependencies]
async-trait = "0.1"
chrono = { version = "0.4", features = ["clock"] }
dashmap = "6"
regex = "1"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
secrecy = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync", "time"] }
tracing = "0.1"
uuid = { version = "1", features = ["v4", "serde"] }
wesichain-core = { path = "../wesichain-core" }
wesichain-graph = { path = "../wesichain-graph" }

[dev-dependencies]
wiremock = "0.6"
```

**Step 3: Create lib.rs with module stubs**

Create `wesichain-langsmith/src/lib.rs`:

```rust
mod client;
mod config;
mod events;
mod exporter;
mod observer;
mod run_store;
mod sampler;
mod sanitize;

pub use client::{LangSmithClient, LangSmithError};
pub use config::LangSmithConfig;
pub use events::{RunEvent, RunStatus, RunType};
pub use exporter::{FlushError, FlushStats, LangSmithExporter};
pub use observer::LangSmithObserver;
pub use run_store::{RunContextStore, RunMetadata, RunUpdateDecision};
pub use sampler::{ProbabilitySampler, Sampler};
pub use sanitize::{ensure_object, sanitize_value, truncate_value};
```

**Step 4: Verify compile**

Run: `cargo test -p wesichain-langsmith`
Expected: PASS

**Step 5: Commit**

```bash
git add Cargo.toml wesichain-langsmith/Cargo.toml wesichain-langsmith/src/lib.rs
git commit -m "feat(langsmith): add crate scaffold"
```

---

### Task 2: Config, sampler, and sanitization helpers

**Files:**
- Create: `wesichain-langsmith/src/config.rs`
- Create: `wesichain-langsmith/src/sampler.rs`
- Create: `wesichain-langsmith/src/sanitize.rs`
- Create: `wesichain-langsmith/tests/sanitize.rs`
- Create: `wesichain-langsmith/tests/sampler.rs`

**Step 1: Write the failing tests**

Create `wesichain-langsmith/tests/sanitize.rs`:

```rust
use regex::Regex;
use serde_json::json;
use wesichain_langsmith::{ensure_object, sanitize_value, truncate_value};

#[test]
fn redaction_applies_before_truncation() {
    let regex = Regex::new("secret").unwrap();
    let value = json!({"token": "secret-secret-secret"});
    let redacted = sanitize_value(value, Some(&regex));
    let truncated = truncate_value(redacted, 10);
    assert_eq!(truncated, json!({"token": "[REDACTED]"}));
}

#[test]
fn non_object_inputs_are_wrapped() {
    let wrapped = ensure_object(json!("hello"));
    assert_eq!(wrapped, json!({"value": "hello"}));
}
```

Create `wesichain-langsmith/tests/sampler.rs`:

```rust
use uuid::Uuid;
use wesichain_langsmith::{ProbabilitySampler, Sampler};

#[test]
fn sampler_is_deterministic_by_run_id() {
    let sampler = ProbabilitySampler { rate: 0.5 };
    let run_id = Uuid::new_v4();
    let first = sampler.should_sample(run_id);
    let second = sampler.should_sample(run_id);
    assert_eq!(first, second);
}

#[test]
fn sampler_respects_bounds() {
    let sampler = ProbabilitySampler { rate: 0.0 };
    assert!(!sampler.should_sample(Uuid::new_v4()));

    let sampler = ProbabilitySampler { rate: 1.0 };
    assert!(sampler.should_sample(Uuid::new_v4()));
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-langsmith sanitize sampler -v`
Expected: FAIL (missing modules/functions)

**Step 3: Implement config, sampler, and sanitize helpers**

Create `wesichain-langsmith/src/config.rs`:

```rust
use std::time::Duration;

use regex::Regex;
use secrecy::SecretString;

#[derive(Clone, Debug)]
pub struct LangSmithConfig {
    pub api_key: SecretString,
    pub api_url: String,
    pub project_name: String,
    pub flush_interval: Duration,
    pub max_batch_size: usize,
    pub queue_capacity: usize,
    pub sampling_rate: f64,
    pub redact_regex: Option<Regex>,
}

impl LangSmithConfig {
    pub fn new(api_key: SecretString, project_name: impl Into<String>) -> Self {
        Self {
            api_key,
            api_url: "https://api.smith.service".to_string(),
            project_name: project_name.into(),
            flush_interval: Duration::from_secs(2),
            max_batch_size: 50,
            queue_capacity: 1000,
            sampling_rate: 1.0,
            redact_regex: None,
        }
    }
}
```

Create `wesichain-langsmith/src/sampler.rs`:

```rust
use uuid::Uuid;

pub trait Sampler: Send + Sync {
    fn should_sample(&self, run_id: Uuid) -> bool;
}

#[derive(Clone, Debug)]
pub struct ProbabilitySampler {
    pub rate: f64,
}

impl Sampler for ProbabilitySampler {
    fn should_sample(&self, run_id: Uuid) -> bool {
        if self.rate <= 0.0 {
            return false;
        }
        if self.rate >= 1.0 {
            return true;
        }
        let bytes = run_id.as_bytes();
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[0..8]);
        let hash = u64::from_le_bytes(buf);
        let ratio = (hash as f64) / (u64::MAX as f64);
        ratio < self.rate
    }
}
```

Create `wesichain-langsmith/src/sanitize.rs`:

```rust
use regex::Regex;
use serde_json::Value;

const REDACTED: &str = "[REDACTED]";

pub fn ensure_object(value: Value) -> Value {
    match value {
        Value::Object(_) => value,
        other => Value::Object(serde_json::Map::from_iter([(
            "value".to_string(),
            other,
        )])),
    }
}

pub fn sanitize_value(value: Value, regex: Option<&Regex>) -> Value {
    match value {
        Value::String(text) => match regex {
            Some(pattern) => Value::String(pattern.replace_all(&text, REDACTED).to_string()),
            None => Value::String(text),
        },
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| sanitize_value(item, regex))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, sanitize_value(value, regex)))
                .collect(),
        ),
        other => other,
    }
}

pub fn truncate_value(value: Value, max_bytes: usize) -> Value {
    match value {
        Value::String(text) => Value::String(truncate_string(&text, max_bytes)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| truncate_value(item, max_bytes))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, truncate_value(value, max_bytes)))
                .collect(),
        ),
        other => other,
    }
}

fn truncate_string(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = 0;
    for (idx, _) in text.char_indices() {
        if idx <= max_bytes {
            end = idx;
        } else {
            break;
        }
    }
    text[..end].to_string()
}
```

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-langsmith sanitize sampler -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-langsmith/src/config.rs wesichain-langsmith/src/sampler.rs wesichain-langsmith/src/sanitize.rs wesichain-langsmith/tests/sanitize.rs wesichain-langsmith/tests/sampler.rs
git commit -m "feat(langsmith): add config, sampler, and sanitization"
```

---

### Task 3: Run events and RunContextStore

**Files:**
- Create: `wesichain-langsmith/src/events.rs`
- Create: `wesichain-langsmith/src/run_store.rs`
- Create: `wesichain-langsmith/tests/run_store.rs`

**Step 1: Write the failing tests**

Create `wesichain-langsmith/tests/run_store.rs`:

```rust
use uuid::Uuid;
use wesichain_langsmith::{RunContextStore, RunStatus};

#[test]
fn first_terminal_event_is_authoritative() {
    let store = RunContextStore::default();
    let run_id = Uuid::new_v4();

    store.record_start(run_id, None);
    let first = store.apply_update(run_id, Some("boom".to_string()));
    let second = store.apply_update(run_id, None);

    assert_eq!(first.status, RunStatus::Failed);
    assert_eq!(second.status, RunStatus::Failed);
    assert_eq!(second.error.as_deref(), Some("boom"));
}

#[test]
fn error_allows_outputs_later() {
    let store = RunContextStore::default();
    let run_id = Uuid::new_v4();

    store.record_start(run_id, None);
    let failed = store.apply_update(run_id, Some("oops".to_string()));
    let later = store.apply_update(run_id, None);

    assert_eq!(failed.status, RunStatus::Failed);
    assert_eq!(later.status, RunStatus::Failed);
    assert_eq!(later.error.as_deref(), Some("oops"));
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-langsmith run_store -v`
Expected: FAIL (missing types)

**Step 3: Implement run events and store**

Create `wesichain-langsmith/src/events.rs`:

```rust
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunType {
    Chain,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug)]
pub enum RunEvent {
    Start {
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        name: String,
        run_type: RunType,
        start_time: DateTime<Utc>,
        inputs: Value,
    },
    Update {
        run_id: Uuid,
        end_time: Option<DateTime<Utc>>,
        outputs: Option<Value>,
        error: Option<String>,
        duration_ms: Option<u128>,
    },
}
```

Create `wesichain-langsmith/src/run_store.rs`:

```rust
use dashmap::DashMap;
use uuid::Uuid;

use crate::events::RunStatus;

#[derive(Clone, Debug)]
pub struct RunMetadata {
    pub status: RunStatus,
    pub error: Option<String>,
    pub parent_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunUpdateDecision {
    pub status: RunStatus,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct RunContextStore {
    runs: DashMap<Uuid, RunMetadata>,
}

impl RunContextStore {
    pub fn record_start(&self, run_id: Uuid, parent_id: Option<Uuid>) {
        self.runs.insert(
            run_id,
            RunMetadata {
                status: RunStatus::Running,
                error: None,
                parent_id,
            },
        );
    }

    pub fn apply_update(&self, run_id: Uuid, error: Option<String>) -> RunUpdateDecision {
        let mut entry = self.runs.entry(run_id).or_insert(RunMetadata {
            status: RunStatus::Running,
            error: None,
            parent_id: None,
        });

        match (&entry.status, error) {
            (RunStatus::Running, Some(err)) => {
                entry.status = RunStatus::Failed;
                entry.error = Some(err.clone());
                RunUpdateDecision {
                    status: RunStatus::Failed,
                    error: Some(err),
                }
            }
            (RunStatus::Running, None) => {
                entry.status = RunStatus::Completed;
                RunUpdateDecision {
                    status: RunStatus::Completed,
                    error: None,
                }
            }
            (RunStatus::Failed, _) => RunUpdateDecision {
                status: RunStatus::Failed,
                error: entry.error.clone(),
            },
            (RunStatus::Completed, Some(_)) => RunUpdateDecision {
                status: RunStatus::Completed,
                error: None,
            },
            (RunStatus::Completed, None) => RunUpdateDecision {
                status: RunStatus::Completed,
                error: None,
            },
        }
    }
}
```

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-langsmith run_store -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-langsmith/src/events.rs wesichain-langsmith/src/run_store.rs wesichain-langsmith/tests/run_store.rs
git commit -m "feat(langsmith): add run events and context store"
```

---

### Task 4: LangSmith HTTP client

**Files:**
- Create: `wesichain-langsmith/src/client.rs`
- Create: `wesichain-langsmith/tests/client.rs`

**Step 1: Write the failing tests**

Create `wesichain-langsmith/tests/client.rs`:

```rust
use secrecy::SecretString;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_langsmith::LangSmithClient;

#[tokio::test]
async fn create_run_includes_idempotency_key() {
    let server = MockServer::start().await;
    let run_id = Uuid::new_v4();
    let payload = json!({"id": run_id, "name": "demo"});

    Mock::given(method("POST"))
        .and(path("/runs"))
        .and(header("x-idempotency-key", run_id.to_string()))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let client = LangSmithClient::new(server.uri(), SecretString::new("test-key".to_string()));
    client.create_run(run_id, &payload).await.unwrap();
}

#[tokio::test]
async fn patch_run_is_partial_payload() {
    let server = MockServer::start().await;
    let run_id = Uuid::new_v4();
    let payload = json!({
        "end_time": "2026-02-03T00:00:00Z",
        "outputs": {"value": 4}
    });

    Mock::given(method("PATCH"))
        .and(path(format!("/runs/{}", run_id)))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let client = LangSmithClient::new(server.uri(), SecretString::new("test-key".to_string()));
    client.update_run(run_id, &payload).await.unwrap();
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-langsmith client -v`
Expected: FAIL (client missing)

**Step 3: Implement LangSmithClient with retries**

Create `wesichain-langsmith/src/client.rs`:

```rust
use std::time::Duration;

use reqwest::{header::HeaderMap, Client, Method, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use thiserror::Error;
use tokio::time::sleep;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum LangSmithError {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("http error: {status}")]
    Http { status: StatusCode, body: String },
}

#[derive(Clone)]
pub struct LangSmithClient {
    client: Client,
    api_url: String,
    api_key: SecretString,
}

impl LangSmithClient {
    pub fn new(api_url: String, api_key: SecretString) -> Self {
        Self {
            client: Client::new(),
            api_url,
            api_key,
        }
    }

    pub async fn create_run(&self, run_id: Uuid, payload: &Value) -> Result<(), LangSmithError> {
        let url = format!("{}/runs", self.api_url.trim_end_matches('/'));
        self.send_with_retry(Method::POST, &url, Some(run_id.to_string()), payload, false)
            .await
    }

    pub async fn update_run(&self, run_id: Uuid, payload: &Value) -> Result<(), LangSmithError> {
        let url = format!("{}/runs/{}", self.api_url.trim_end_matches('/'), run_id);
        self.send_with_retry(Method::PATCH, &url, None, payload, true)
            .await
    }

    async fn send_with_retry(
        &self,
        method: Method,
        url: &str,
        idempotency_key: Option<String>,
        payload: &Value,
        allow_not_found: bool,
    ) -> Result<(), LangSmithError> {
        let mut attempt = 0;
        let mut backoff = Duration::from_millis(200);

        loop {
            attempt += 1;
            let mut request = self
                .client
                .request(method.clone(), url)
                .header("x-api-key", self.api_key.expose_secret())
                .json(payload);
            if let Some(key) = &idempotency_key {
                request = request.header("x-idempotency-key", key);
            }

            match request.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(());
                    }
                    if allow_not_found && response.status() == StatusCode::NOT_FOUND {
                        return Ok(());
                    }
                    if should_retry(response.status()) && attempt < 3 {
                        backoff = next_delay(response.status(), response.headers(), backoff);
                        sleep(backoff).await;
                        continue;
                    }
                    let body = response.text().await.unwrap_or_default();
                    return Err(LangSmithError::Http {
                        status: response.status(),
                        body,
                    });
                }
                Err(err) => {
                    if (err.is_timeout() || err.is_connect()) && attempt < 3 {
                        sleep(backoff).await;
                        backoff = backoff.saturating_mul(2);
                        continue;
                    }
                    return Err(LangSmithError::Request(err));
                }
            }
        }
    }
}

fn should_retry(status: StatusCode) -> bool {
    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
}

fn next_delay(status: StatusCode, headers: &HeaderMap, backoff: Duration) -> Duration {
    if status == StatusCode::TOO_MANY_REQUESTS {
        if let Some(value) = headers.get("Retry-After").and_then(|v| v.to_str().ok()) {
            if let Ok(seconds) = value.parse::<u64>() {
                return Duration::from_secs(seconds);
            }
        }
    }
    backoff.saturating_mul(2)
}
```

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-langsmith client -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-langsmith/src/client.rs wesichain-langsmith/tests/client.rs
git commit -m "feat(langsmith): add LangSmith HTTP client"
```

---

### Task 5: Exporter with batching and backpressure

**Files:**
- Create: `wesichain-langsmith/src/exporter.rs`
- Create: `wesichain-langsmith/tests/exporter.rs`

**Step 1: Write the failing tests**

Create `wesichain-langsmith/tests/exporter.rs`:

```rust
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use secrecy::SecretString;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_langsmith::{
    LangSmithConfig, LangSmithExporter, RunContextStore, RunEvent, RunType,
};

#[tokio::test]
async fn drops_oldest_when_queue_full() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/runs"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let config = LangSmithConfig {
        api_key: SecretString::new("key".to_string()),
        api_url: server.uri(),
        project_name: "test".to_string(),
        flush_interval: Duration::from_secs(3600),
        max_batch_size: 10,
        queue_capacity: 1,
        sampling_rate: 1.0,
        redact_regex: None,
    };
    let exporter = LangSmithExporter::new(config, Arc::new(RunContextStore::default()));

    exporter
        .enqueue(RunEvent::Start {
            run_id: Uuid::new_v4(),
            parent_run_id: None,
            name: "a".to_string(),
            run_type: RunType::Chain,
            start_time: Utc::now(),
            inputs: json!({}),
        })
        .await;
    exporter
        .enqueue(RunEvent::Start {
            run_id: Uuid::new_v4(),
            parent_run_id: None,
            name: "b".to_string(),
            run_type: RunType::Chain,
            start_time: Utc::now(),
            inputs: json!({}),
        })
        .await;

    assert_eq!(exporter.dropped_events(), 1);
}

#[tokio::test]
async fn flushes_on_batch_size() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/runs"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let config = LangSmithConfig {
        api_key: SecretString::new("key".to_string()),
        api_url: server.uri(),
        project_name: "test".to_string(),
        flush_interval: Duration::from_secs(3600),
        max_batch_size: 2,
        queue_capacity: 10,
        sampling_rate: 1.0,
        redact_regex: None,
    };
    let exporter = LangSmithExporter::new(config, Arc::new(RunContextStore::default()));

    exporter
        .enqueue(RunEvent::Start {
            run_id: Uuid::new_v4(),
            parent_run_id: None,
            name: "a".to_string(),
            run_type: RunType::Chain,
            start_time: Utc::now(),
            inputs: json!({}),
        })
        .await;
    exporter
        .enqueue(RunEvent::Start {
            run_id: Uuid::new_v4(),
            parent_run_id: None,
            name: "b".to_string(),
            run_type: RunType::Chain,
            start_time: Utc::now(),
            inputs: json!({}),
        })
        .await;

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if exporter.pending_len().await == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("batch flush did not drain queue");
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-langsmith exporter -v`
Expected: FAIL (missing exporter)

**Step 3: Implement exporter**

Create `wesichain-langsmith/src/exporter.rs`:

```rust
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde_json::{json, Map, Value};
use tokio::sync::{Mutex, Notify};

use crate::{
    LangSmithClient, LangSmithConfig, LangSmithError, RunContextStore, RunEvent, RunType,
};

#[derive(Clone, Debug, Default)]
pub struct FlushStats {
    pub runs_flushed: usize,
    pub runs_failed: usize,
    pub batches_sent: usize,
    pub dropped_events: u64,
}

#[derive(Debug)]
pub enum FlushError {
    Timeout { waited: Duration, pending: usize },
    Permanent { reason: String, batch_dropped: usize },
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

    #[cfg(test)]
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
                self.client.update_run(*run_id, &Value::Object(payload)).await
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
```

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-langsmith exporter -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-langsmith/src/exporter.rs wesichain-langsmith/tests/exporter.rs
git commit -m "feat(langsmith): add batch exporter"
```

---

### Task 6: LangSmithObserver implementation

**Files:**
- Create: `wesichain-langsmith/src/observer.rs`
- Create: `wesichain-langsmith/tests/observer.rs`

**Step 1: Write the failing tests**

Create `wesichain-langsmith/tests/observer.rs`:

```rust
use std::sync::Arc;
use std::time::Duration;

use secrecy::SecretString;
use serde_json::json;
use uuid::Uuid;

use wesichain_langsmith::{LangSmithConfig, LangSmithObserver, Sampler};

struct NeverSampler;

impl Sampler for NeverSampler {
    fn should_sample(&self, _run_id: Uuid) -> bool {
        false
    }
}

#[tokio::test]
async fn sampling_short_circuits_before_enqueue() {
    let config = LangSmithConfig {
        api_key: SecretString::new("key".to_string()),
        api_url: "http://localhost".to_string(),
        project_name: "test".to_string(),
        flush_interval: Duration::from_secs(3600),
        max_batch_size: 10,
        queue_capacity: 10,
        sampling_rate: 1.0,
        redact_regex: None,
    };
    let observer = LangSmithObserver::with_sampler(config, Arc::new(NeverSampler));

    observer.on_node_start("node", &json!({"x": 1})).await;
    let stats = observer.flush(Duration::from_millis(50)).await.unwrap();
    assert_eq!(stats.runs_flushed, 0);
}

#[tokio::test]
async fn dropped_events_counter_increments() {
    let config = LangSmithConfig {
        api_key: SecretString::new("key".to_string()),
        api_url: "http://localhost".to_string(),
        project_name: "test".to_string(),
        flush_interval: Duration::from_secs(3600),
        max_batch_size: 10,
        queue_capacity: 1,
        sampling_rate: 1.0,
        redact_regex: None,
    };
    let observer = LangSmithObserver::new(config);

    observer.on_node_start("node-a", &json!({"x": 1})).await;
    observer.on_node_start("node-b", &json!({"x": 2})).await;

    assert_eq!(observer.dropped_events(), 1);
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p wesichain-langsmith observer -v`
Expected: FAIL (observer missing)

**Step 3: Implement LangSmithObserver**

Create `wesichain-langsmith/src/observer.rs`:

```rust
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

    pub async fn flush(&self, timeout: std::time::Duration) -> Result<crate::FlushStats, crate::FlushError> {
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
        self.tool_runs.get_mut(key).and_then(|mut entry| entry.pop_front())
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
```

**Step 4: Run tests to verify pass**

Run: `cargo test -p wesichain-langsmith observer -v`
Expected: PASS

**Step 5: Commit**

```bash
git add wesichain-langsmith/src/observer.rs wesichain-langsmith/tests/observer.rs
git commit -m "feat(langsmith): implement observer"
```

---

### Task 7: Integration test with ReActAgentNode

**Files:**
- Create: `wesichain-langsmith/tests/react_agent_integration.rs`

**Step 1: Write the failing test**

Create `wesichain-langsmith/tests/react_agent_integration.rs`:

```rust
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use secrecy::SecretString;
use uuid::Uuid;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_core::{
    HasFinalOutput, HasUserInput, LlmRequest, LlmResponse, ReActStep, ScratchpadState, Tool,
    ToolCall, ToolCallingLlm, ToolError, Value, WesichainError,
};
use wesichain_graph::{ExecutionOptions, GraphBuilder, GraphState, ReActAgentNode, StateSchema};
use wesichain_langsmith::{LangSmithConfig, LangSmithObserver};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    input: String,
    scratchpad: Vec<ReActStep>,
    final_output: Option<String>,
    iterations: u32,
}

impl StateSchema for DemoState {}

impl ScratchpadState for DemoState {
    fn scratchpad(&self) -> &Vec<ReActStep> {
        &self.scratchpad
    }

    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> {
        &mut self.scratchpad
    }

    fn iteration_count(&self) -> u32 {
        self.iterations
    }

    fn increment_iteration(&mut self) {
        self.iterations += 1;
    }
}

impl HasUserInput for DemoState {
    fn user_input(&self) -> &str {
        &self.input
    }
}

impl HasFinalOutput for DemoState {
    fn final_output(&self) -> Option<&str> {
        self.final_output.as_deref()
    }

    fn set_final_output(&mut self, value: String) {
        self.final_output = Some(value);
    }
}

struct MockTool;

#[async_trait::async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "math"
    }

    fn schema(&self) -> Value {
        json!({"type": "object"})
    }

    async fn invoke(&self, _args: Value) -> Result<Value, ToolError> {
        Ok(json!(4))
    }
}

struct MockLlm;

#[async_trait::async_trait]
impl ToolCallingLlm for MockLlm {
    async fn invoke(&self, _request: LlmRequest) -> Result<LlmResponse, WesichainError> {
        Ok(LlmResponse {
            content: "".to_string(),
            tool_calls: vec![ToolCall {
                id: Uuid::new_v4().to_string(),
                name: "calculator".to_string(),
                args: json!({"expression": "2+2"}),
            }],
        })
    }
}

#[tokio::test]
async fn langsmith_traces_react_agent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex("/runs"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path_regex("/runs/.*"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let config = LangSmithConfig {
        api_key: SecretString::new("key".to_string()),
        api_url: server.uri(),
        project_name: "test".to_string(),
        flush_interval: Duration::from_secs(3600),
        max_batch_size: 50,
        queue_capacity: 1000,
        sampling_rate: 1.0,
        redact_regex: None,
    };

    let observer = Arc::new(LangSmithObserver::new(config));
    let options = ExecutionOptions {
        observer: Some(observer.clone()),
        ..Default::default()
    };

    let node = ReActAgentNode::builder()
        .llm(Arc::new(MockLlm))
        .tools(vec![Arc::new(MockTool)])
        .max_iterations(1)
        .build()
        .unwrap();

    let graph = GraphBuilder::new()
        .add_node("agent", node)
        .set_entry("agent")
        .build();

    let state = GraphState::new(DemoState {
        input: "2+2".to_string(),
        ..Default::default()
    });

    let _ = graph.invoke_with_options(state, options).await.unwrap();
    let stats = observer.flush(Duration::from_secs(5)).await.unwrap();

    assert!(stats.runs_flushed > 0);
    let requests = server.received_requests().await.unwrap();
    assert!(requests.iter().any(|req| req.method == "POST"));
    assert!(requests.iter().any(|req| req.method == "PATCH"));
}
```

**Step 2: Run test to verify failure**

Run: `cargo test -p wesichain-langsmith react_agent_integration -v`
Expected: FAIL (observer/exporter missing)

**Step 3: Run test to verify pass**

Run: `cargo test -p wesichain-langsmith react_agent_integration -v`
Expected: PASS

**Step 4: Commit**

```bash
git add wesichain-langsmith/tests/react_agent_integration.rs
git commit -m "test(langsmith): add react agent integration"
```

---

### Task 8: Documentation and usage example

**Files:**
- Modify: `wesichain-langsmith/src/lib.rs`

**Step 1: Add crate-level docs with example**

Update `wesichain-langsmith/src/lib.rs` (add to top):

```rust
//! LangSmith observability for Wesichain graphs.
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! use secrecy::SecretString;
//! use wesichain_graph::{ExecutionOptions, ExecutableGraph, GraphState, StateSchema};
//! use wesichain_langsmith::{LangSmithConfig, LangSmithObserver};
//!
//! #[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
//! struct DemoState;
//!
//! impl StateSchema for DemoState {}
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = LangSmithConfig {
//!         api_key: SecretString::new("key".to_string()),
//!         api_url: "https://api.smith.service".to_string(),
//!         project_name: "example".to_string(),
//!         flush_interval: Duration::from_secs(2),
//!         max_batch_size: 50,
//!         queue_capacity: 1000,
//!         sampling_rate: 1.0,
//!         redact_regex: None,
//!     };
//!
//!     let observer = Arc::new(LangSmithObserver::new(config));
//!     let options = ExecutionOptions {
//!         observer: Some(observer.clone()),
//!         ..Default::default()
//!     };
//!
//!     let graph: ExecutableGraph<DemoState> = todo!("build with GraphBuilder");
//!     let state = GraphState::new(DemoState::default());
//!     let _ = graph.invoke_with_options(state, options).await;
//!     let _ = observer.flush(Duration::from_secs(5)).await;
//! }
//! ```
```

**Step 2: Verify docs build**

Run: `cargo test -p wesichain-langsmith --doc`
Expected: PASS

**Step 3: Commit**

```bash
git add wesichain-langsmith/src/lib.rs
git commit -m "docs(langsmith): add usage example"
```

---

## Validation Checklist
- [ ] Observer trait signature matches Phase 5 (non-generic, JSON inputs/outputs).
- [ ] Node/tool events produce parent/child runs with correct IDs.
- [ ] Redaction runs before truncation and is applied per field.
- [ ] Non-object inputs/outputs are wrapped as `{ "value": ... }`.
- [ ] Drop-oldest backpressure increments `dropped_events` without blocking.
- [ ] POST uses `x-idempotency-key` and PATCH is partial.
- [ ] Integration test with `ReActAgentNode` produces POST and PATCH requests.
