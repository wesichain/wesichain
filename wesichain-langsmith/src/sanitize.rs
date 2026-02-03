use regex::Regex;
use serde_json::Value;

pub fn ensure_object(value: Value) -> Value {
    value
}

pub fn sanitize_value(value: Value, _regex: Option<&Regex>) -> Value {
    value
}

pub fn truncate_value(value: Value, _max_bytes: usize) -> Value {
    value
}
