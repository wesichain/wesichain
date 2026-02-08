use futures::StreamExt;
use wesichain_core::AgentEvent;
use wesichain_rag::{RagQueryRequest, WesichainRag};

#[tokio::test]
async fn builder_defaults_to_bounded_event_buffer() {
    let rag = WesichainRag::builder()
        .build()
        .expect("default builder should produce a facade instance");

    assert_eq!(rag.event_buffer_size(), 64);
}

#[tokio::test]
async fn query_stream_uses_provided_thread_id_and_monotonic_steps() {
    let rag = WesichainRag::builder()
        .build()
        .expect("facade should build");

    let stream = rag
        .query_stream(RagQueryRequest {
            query: "What is wesichain?".to_string(),
            thread_id: Some("thread-fixed".to_string()),
        })
        .await
        .expect("query_stream should succeed");

    let events: Vec<AgentEvent> = stream
        .map(|item| item.expect("stream should not emit errors"))
        .collect()
        .await;

    assert!(!events.is_empty(), "expected at least one AgentEvent");

    let mut saw_thread_id = false;
    let steps: Vec<usize> = events.iter().filter_map(AgentEvent::step).collect();
    for event in events {
        if let AgentEvent::Status { thread_id, .. } = event {
            assert_eq!(thread_id, "thread-fixed");
            saw_thread_id = true;
        }
    }

    assert!(saw_thread_id, "expected at least one status event");
    assert!(
        steps.windows(2).all(|window| window[1] > window[0]),
        "stream steps must be strictly increasing"
    );
}

#[tokio::test]
async fn query_stream_generates_thread_id_when_missing() {
    let rag = WesichainRag::builder()
        .build()
        .expect("facade should build");

    let mut stream = rag
        .query_stream(RagQueryRequest {
            query: "Explain checkpoints".to_string(),
            thread_id: None,
        })
        .await
        .expect("query_stream should succeed");

    let first = stream
        .next()
        .await
        .expect("stream should emit at least one event")
        .expect("event should be ok");

    match first {
        AgentEvent::Status { thread_id, .. } => {
            assert!(!thread_id.trim().is_empty());
        }
        other => panic!("expected first event to be status, got {other:?}"),
    }
}
