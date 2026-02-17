use wesichain_core::{LlmInput, LlmResult, TokenUsage};

#[test]
fn token_usage_creation() {
    let usage = TokenUsage {
        prompt_tokens: 10,
        completion_tokens: 20,
        total_tokens: 30,
    };
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 20);
    assert_eq!(usage.total_tokens, 30);
}

#[test]
fn llm_input_creation() {
    let input = LlmInput {
        model: "gpt-4".to_string(),
        prompt: "Hello, world!".to_string(),
        temperature: Some(0.7),
        max_tokens: Some(100),
        stop_sequences: vec!["\n".to_string()],
    };
    assert_eq!(input.model, "gpt-4");
    assert_eq!(input.prompt, "Hello, world!");
    assert_eq!(input.temperature, Some(0.7));
    assert_eq!(input.max_tokens, Some(100));
    assert_eq!(input.stop_sequences, vec!["\n"]);
}

#[test]
fn llm_result_with_token_usage() {
    let result = LlmResult {
        token_usage: Some(TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        }),
        model: "gpt-4".to_string(),
        finish_reason: Some("stop".to_string()),
        generations: vec!["Hi there!".to_string()],
    };
    assert!(result.token_usage.is_some());
    assert_eq!(result.token_usage.as_ref().unwrap().total_tokens, 30);
}

#[test]
fn llm_result_without_token_usage() {
    let result = LlmResult {
        token_usage: None,
        model: "local-model".to_string(),
        finish_reason: None,
        generations: vec![],
    };
    assert!(result.token_usage.is_none());
}
