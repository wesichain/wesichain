use std::sync::Arc;
use std::time::Duration;

use secrecy::SecretString;
use serde_json::json;
use uuid::Uuid;
use wesichain_core::callbacks::{CallbackHandler, RunContext, RunType};

use wesichain_langsmith::{LangSmithCallbackHandler, LangSmithConfig, Sampler};

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
    let handler = LangSmithCallbackHandler::with_sampler(config, Arc::new(NeverSampler));
    let ctx = RunContext::root(RunType::Chain, "node".to_string(), vec![], Default::default());
    handler.on_start(&ctx, &json!({"x": 1})).await;
    let stats = handler.flush(Duration::from_millis(50)).await.unwrap();
    assert_eq!(stats.runs_flushed, 0);
}
