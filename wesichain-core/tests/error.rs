use std::{error::Error, time::Duration};

use wesichain_core::{EmbeddingError, WesichainError};

#[test]
fn error_display_for_max_retries() {
    let err = WesichainError::MaxRetriesExceeded { max: 2 };
    assert_eq!(format!("{err}"), "Max retries (2) exceeded");
}

#[test]
fn error_display_for_llm_provider() {
    let err = WesichainError::LlmProvider("rate limited".to_string());
    assert_eq!(format!("{err}"), "LLM provider failed: rate limited");
}

#[test]
fn error_display_for_tool_call_failed() {
    let err = WesichainError::ToolCallFailed {
        tool_name: "search".to_string(),
        reason: "timeout".to_string(),
    };
    assert_eq!(format!("{err}"), "Tool call failed for 'search': timeout");
}

#[test]
fn error_display_for_parse_failed() {
    let err = WesichainError::ParseFailed {
        output: "<html>".to_string(),
        reason: "unexpected token".to_string(),
    };
    assert_eq!(
        format!("{err}"),
        "Parsing failed on output '<html>': unexpected token"
    );
}

#[test]
fn error_display_for_timeout() {
    let err = WesichainError::Timeout(Duration::from_secs(5));
    assert_eq!(format!("{err}"), "Operation timed out after 5s");
}

#[test]
fn error_display_for_checkpoint_failed() {
    let err = WesichainError::CheckpointFailed("checksum mismatch".to_string());
    assert_eq!(format!("{err}"), "Checkpoint failed: checksum mismatch");
}

#[test]
fn error_display_for_cancelled() {
    let err = WesichainError::Cancelled;
    assert_eq!(format!("{err}"), "Operation was cancelled");
}

#[test]
fn error_display_for_invalid_config() {
    let err = WesichainError::InvalidConfig("missing api key".to_string());
    assert_eq!(format!("{err}"), "Invalid configuration: missing api key");
}

#[test]
fn error_display_for_serde() {
    let parse_error = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
    let err = WesichainError::Serde(parse_error);
    assert!(format!("{err}").starts_with("Serialization/deserialization error: "));
}

#[test]
fn error_display_for_custom() {
    let err = WesichainError::Custom("something odd".to_string());
    assert_eq!(format!("{err}"), "something odd");
}

#[test]
fn embedding_error_display_for_rate_limited() {
    let err = EmbeddingError::RateLimited {
        retry_after: Some(Duration::from_secs(2)),
    };
    assert_eq!(format!("{err}"), "rate limited (retry_after=2s)");
}

#[test]
fn embedding_error_display_for_timeout() {
    let err = EmbeddingError::Timeout(Duration::from_millis(750));
    assert_eq!(format!("{err}"), "timeout after 750ms");
}

#[test]
fn embedding_error_display_for_other() {
    let err = EmbeddingError::Other("network".to_string().into());
    assert_eq!(format!("{err}"), "Embedding error: network");
    assert!(err.source().is_some());
}
