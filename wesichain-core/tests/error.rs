use std::time::Duration;

use wesichain_core::WesichainError;

#[test]
fn error_display_for_max_retries() {
    let err = WesichainError::MaxRetriesExceeded { max: 2 };
    assert_eq!(format!("{err}"), "Max retries (2) exceeded");
}

#[test]
fn error_display_for_llm_provider() {
    let err = WesichainError::LlmProvider("rate limited".to_string());
    assert_eq!(format!("{err}"), "LLM provider error: rate limited");
}

#[test]
fn error_display_for_tool_call_failed() {
    let err = WesichainError::ToolCallFailed {
        tool_name: "search".to_string(),
        reason: "timeout".to_string(),
    };
    assert_eq!(format!("{err}"), "Tool call failed (search): timeout");
}

#[test]
fn error_display_for_parse_failed() {
    let err = WesichainError::ParseFailed {
        output: "<html>".to_string(),
        reason: "unexpected token".to_string(),
    };
    assert_eq!(
        format!("{err}"),
        "Parse failed: unexpected token. Output: <html>"
    );
}

#[test]
fn error_display_for_timeout() {
    let err = WesichainError::Timeout(Duration::from_secs(5));
    assert_eq!(format!("{err}"), "Timeout after 5s");
}

#[test]
fn error_display_for_checkpoint_failed() {
    let err = WesichainError::CheckpointFailed("checksum mismatch".to_string());
    assert_eq!(format!("{err}"), "Checkpoint failed: checksum mismatch");
}

#[test]
fn error_display_for_cancelled() {
    let err = WesichainError::Cancelled;
    assert_eq!(format!("{err}"), "Operation cancelled");
}

#[test]
fn error_display_for_invalid_config() {
    let err = WesichainError::InvalidConfig("missing api key".to_string());
    assert_eq!(format!("{err}"), "Invalid config: missing api key");
}

#[test]
fn error_display_for_serde() {
    let parse_error = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
    let err = WesichainError::Serde(parse_error);
    assert!(format!("{err}").starts_with("Serde error: "));
}

#[test]
fn error_display_for_custom() {
    let err = WesichainError::Custom("something odd".to_string());
    assert_eq!(format!("{err}"), "Custom error: something odd");
}
