use std::sync::Arc;
use std::time::Duration;

use secrecy::SecretString;
use serde_json::json;
use uuid::Uuid;

use wesichain_graph::Observer;
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
