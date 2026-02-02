use wesichain_core::StreamEvent;
use wesichain_llm::ollama_stream_events;

#[test]
fn parse_stream_lines_into_events() {
    let input = br#"{\"message\":{\"content\":\"Hel\"},\"done\":false}\n{\"message\":{\"content\":\"lo\"},\"done\":false}\n{\"message\":{\"content\":\"!\"},\"done\":true}"#;
    let events = ollama_stream_events(input).expect("parse");
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], StreamEvent::ContentChunk(_)));
    assert!(matches!(events[2], StreamEvent::FinalAnswer(_)));
}

#[test]
fn parse_stream_rejects_malformed_json() {
    let bad = br#"{\"message\":{\"content\":\"hi\"}"#;
    assert!(ollama_stream_events(bad).is_err());
}
