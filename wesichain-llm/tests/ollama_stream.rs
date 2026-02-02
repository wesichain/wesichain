use wesichain_core::StreamEvent;
use wesichain_llm::ollama_stream_events;

#[test]
fn parse_stream_lines_into_events() {
    let input = br#"{"message":{"content":"Hel"},"done":false}
{"message":{"content":"lo"},"done":false}
{"message":{"content":"!"},"done":true}"#;
    let events = ollama_stream_events(input).expect("parse");
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], StreamEvent::ContentChunk(ref content) if content == "Hel"));
    assert!(matches!(events[1], StreamEvent::ContentChunk(ref content) if content == "lo"));
    assert!(matches!(events[2], StreamEvent::FinalAnswer(ref content) if content == "!"));
}

#[test]
fn parse_stream_rejects_malformed_json() {
    let bad = br#"{\"message\":{\"content\":\"hi\"}"#;
    assert!(ollama_stream_events(bad).is_err());
}

#[test]
fn parse_stream_rejects_escaped_ndjson() {
    let escaped = br#"{\"message\":{\"content\":\"Hel\"},\"done\":false}\n{\"message\":{\"content\":\"lo\"},\"done\":true}"#;
    assert!(ollama_stream_events(escaped).is_err());
}
