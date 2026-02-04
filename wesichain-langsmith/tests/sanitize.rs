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
