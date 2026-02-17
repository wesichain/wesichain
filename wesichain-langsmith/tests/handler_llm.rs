use std::collections::BTreeMap;
use std::time::Duration;

use secrecy::SecretString;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wesichain_core::{CallbackHandler, LlmInput, LlmResult, RunContext, RunType, TokenUsage};
use wesichain_langsmith::{LangSmithCallbackHandler, LangSmithConfig};

#[tokio::test]
async fn handler_emits_llm_start_event() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/runs"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&mock_server)
        .await;

    let config = LangSmithConfig {
        api_key: SecretString::new("test-key".to_string()),
        api_url: mock_server.uri(),
        project_name: "test-project".to_string(),
        flush_interval: Duration::from_secs(1),
        max_batch_size: 10,
        queue_capacity: 100,
        sampling_rate: 1.0,
        redact_regex: None,
    };

    let handler = LangSmithCallbackHandler::new(config);

    let ctx = RunContext::root(RunType::Llm, "test-llm".to_string(), vec![], BTreeMap::new());
    let input = LlmInput {
        model: "gpt-4".to_string(),
        prompt: "Hello".to_string(),
        temperature: Some(0.7),
        max_tokens: Some(100),
        stop_sequences: vec![],
    };

    handler.on_llm_start(&ctx, &input).await;

    // Flush to ensure event is sent
    let _ = handler.flush(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn handler_emits_llm_end_with_token_usage() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/runs"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&mock_server)
        .await;

    Mock::given(method("PATCH"))
        .and(path("/runs/.+"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&mock_server)
        .await;

    let config = LangSmithConfig {
        api_key: SecretString::new("test-key".to_string()),
        api_url: mock_server.uri(),
        project_name: "test-project".to_string(),
        flush_interval: Duration::from_secs(1),
        max_batch_size: 10,
        queue_capacity: 100,
        sampling_rate: 1.0,
        redact_regex: None,
    };

    let handler = LangSmithCallbackHandler::new(config);

    // First call on_llm_start
    let ctx = RunContext::root(RunType::Llm, "test-llm".to_string(), vec![], BTreeMap::new());
    let input = LlmInput {
        model: "gpt-4".to_string(),
        prompt: "Hello".to_string(),
        temperature: None,
        max_tokens: None,
        stop_sequences: vec![],
    };
    handler.on_llm_start(&ctx, &input).await;

    // Then call on_llm_end with token usage
    let result = LlmResult {
        token_usage: Some(TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        }),
        model: "gpt-4".to_string(),
        finish_reason: Some("stop".to_string()),
        generations: vec!["Hi".to_string()],
    };
    handler.on_llm_end(&ctx, &result, 100).await;

    let _ = handler.flush(Duration::from_secs(1)).await;
}
