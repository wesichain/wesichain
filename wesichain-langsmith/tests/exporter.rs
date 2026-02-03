use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use secrecy::SecretString;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_langsmith::{LangSmithConfig, LangSmithExporter, RunContextStore, RunEvent, RunType};

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
