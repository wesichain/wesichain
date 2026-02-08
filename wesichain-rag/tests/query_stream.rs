use futures::StreamExt;
use wesichain_core::AgentEvent;
use wesichain_rag::{RagQueryRequest, WesichainRag};

async fn collect_events(
    rag: &WesichainRag,
    query: &str,
    thread_id: Option<&str>,
) -> Vec<AgentEvent> {
    let stream = rag
        .query_stream(RagQueryRequest {
            query: query.to_string(),
            thread_id: thread_id.map(ToString::to_string),
        })
        .await
        .expect("query_stream should succeed");

    stream
        .map(|item| item.expect("event should be ok"))
        .collect::<Vec<_>>()
        .await
}

#[tokio::test]
async fn query_stream_emits_graph_status_events_then_final_answer() {
    let rag = WesichainRag::builder()
        .build()
        .expect("facade should build");
    let events = collect_events(&rag, "What is checkpointing?", Some("thread-qs-1")).await;

    assert!(
        events.iter().any(|event| matches!(
            event,
            AgentEvent::Status { stage, .. } if stage == "node_start"
        )),
        "expected node_start status event from graph"
    );

    let final_answer = events.iter().find_map(|event| match event {
        AgentEvent::Final { content, .. } => Some(content.as_str()),
        _ => None,
    });
    assert_eq!(
        final_answer,
        Some("Stub answer #1 for: What is checkpointing?"),
        "expected first turn answer to include turn counter"
    );
}

#[tokio::test]
async fn query_stream_resumes_state_for_same_thread_id() {
    let rag = WesichainRag::builder()
        .build()
        .expect("facade should build");

    let first = collect_events(&rag, "First question", Some("thread-resume-1")).await;
    let second = collect_events(&rag, "Second question", Some("thread-resume-1")).await;

    let first_final = first.iter().find_map(|event| match event {
        AgentEvent::Final { content, .. } => Some(content.as_str()),
        _ => None,
    });
    let second_final = second.iter().find_map(|event| match event {
        AgentEvent::Final { content, .. } => Some(content.as_str()),
        _ => None,
    });

    assert_eq!(first_final, Some("Stub answer #1 for: First question"));
    assert_eq!(second_final, Some("Stub answer #2 for: Second question"));
}
