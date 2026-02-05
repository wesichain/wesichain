use regex::Regex;
use serde_json::Value;

const REDACTED: &str = "[REDACTED]";

pub fn ensure_object(value: Value) -> Value {
    match value {
        Value::Object(_) => value,
        other => Value::Object(serde_json::Map::from_iter([("value".to_string(), other)])),
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
