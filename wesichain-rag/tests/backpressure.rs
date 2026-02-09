use std::time::Duration;

use futures::StreamExt;
use tokio::time::{sleep, timeout};
use wesichain_core::{AgentEvent, Document};
use wesichain_rag::{RagQueryRequest, WesichainRag};

fn demo_docs() -> Vec<Document> {
    vec![
        Document {
            id: "bp-doc-1".to_string(),
            content: "Backpressure testing ensures streaming reliability.".to_string(),
            metadata: Default::default(),
            embedding: None,
        },
        Document {
            id: "bp-doc-2".to_string(),
            content: "Semantic events must be preserved under load.".to_string(),
            metadata: Default::default(),
            embedding: None,
        },
    ]
}

#[tokio::test]
async fn stream_respects_backpressure_semantic_events_preserved() {
    let rag = WesichainRag::builder().build().expect("rag should build");

    rag.add_documents(demo_docs())
        .await
        .expect("documents should index");

    let stream = rag
        .query_stream(RagQueryRequest {
            query: "What about backpressure?".to_string(),
            thread_id: Some("backpressure-test".to_string()),
        })
        .await
        .expect("stream should start");

    let mut semantic_events = 0usize;
    let mut final_received = false;
    let mut _error_events = 0usize;

    tokio::pin!(stream);

    while let Some(result) = stream.next().await {
        let event = result.expect("stream should not error");

        match event {
            AgentEvent::Status { .. } => semantic_events += 1,
            AgentEvent::Thought { .. } => semantic_events += 1,
            AgentEvent::ToolCall { .. } => semantic_events += 1,
            AgentEvent::Observation { .. } => semantic_events += 1,
            AgentEvent::Final { .. } => {
                semantic_events += 1;
                final_received = true;
            }
            AgentEvent::Error { recoverable, .. } => {
                _error_events += 1;
                if !recoverable {
                    break;
                }
            }
            AgentEvent::Metadata { .. } => {}
        }

        sleep(Duration::from_millis(50)).await;
    }

    assert!(
        semantic_events >= 3,
        "expected at least 3 semantic events (status events + final), got {semantic_events}"
    );
    assert!(
        final_received,
        "stream should terminate with Final event under backpressure"
    );
}

#[tokio::test]
async fn stream_terminates_cleanly_with_timeout() {
    let rag = WesichainRag::builder().build().expect("rag should build");

    rag.add_documents(demo_docs())
        .await
        .expect("documents should index");

    let stream = rag
        .query_stream(RagQueryRequest {
            query: "Test timeout termination".to_string(),
            thread_id: Some("timeout-test".to_string()),
        })
        .await
        .expect("stream should start");

    let result: Result<Vec<AgentEvent>, _> = timeout(Duration::from_secs(5), async {
        let mut events = vec![];
        tokio::pin!(stream);

        while let Some(item) = stream.next().await {
            events.push(item.expect("item should be ok"));
        }

        events
    })
    .await;

    match result {
        Ok(events) => {
            assert!(
                events.iter().any(|e| matches!(e, AgentEvent::Final { .. })),
                "stream should complete with Final event"
            );
        }
        Err(_) => panic!("stream should complete within timeout"),
    }
}

#[tokio::test]
async fn multiple_concurrent_streams_isolated() {
    let rag = WesichainRag::builder().build().expect("rag should build");

    rag.add_documents(demo_docs())
        .await
        .expect("documents should index");

    let mut handles = vec![];

    for i in 0..3 {
        let rag_clone = rag.clone();
        let handle = tokio::spawn(async move {
            let stream = rag_clone
                .query_stream(RagQueryRequest {
                    query: format!("Concurrent query {}", i),
                    thread_id: Some(format!("concurrent-thread-{}", i)),
                })
                .await
                .expect("stream should start");

            let events: Vec<AgentEvent> = stream.filter_map(|r| async { r.ok() }).collect().await;

            let has_final = events.iter().any(|e| matches!(e, AgentEvent::Final { .. }));
            assert!(has_final, "stream {} should complete with Final", i);
            events.len()
        });
        handles.push(handle);
    }

    let results = futures::future::join_all(handles).await;
    let total_events: usize = results.into_iter().map(|r| r.unwrap()).sum();

    assert!(
        total_events >= 9,
        "expected at least 9 total events across 3 streams, got {total_events}"
    );
}
