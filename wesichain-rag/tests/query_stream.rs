use futures::StreamExt;
use wesichain_core::{AgentEvent, Document};
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

async fn setup_rag_with_docs() -> WesichainRag {
    let rag = WesichainRag::builder()
        .build()
        .expect("facade should build");

    // Add test documents for retrieval
    let docs = vec![
        Document {
            id: "doc-1".to_string(),
            content: "Checkpointing is a technique used in database systems and distributed computing to save the state of a system periodically. This allows for recovery in case of failures.".to_string(),
            metadata: Default::default(),
            embedding: None,
        },
        Document {
            id: "doc-2".to_string(),
            content: "Wesichain is a Rust-native framework for building LLM agents with resumable workflows and stateful graph execution.".to_string(),
            metadata: Default::default(),
            embedding: None,
        },
    ];

    rag.add_documents(docs).await.expect("should add documents");
    rag
}

#[tokio::test]
async fn query_stream_emits_graph_status_events_then_final_answer() {
    let rag = setup_rag_with_docs().await;
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
    // The answer should now include the retrieved context
    assert!(final_answer.is_some(), "expected a final answer");
    let answer = final_answer.unwrap();
    assert!(
        answer.contains("Stub answer #1"),
        "expected answer to include turn counter, got: {}",
        answer
    );
    assert!(
        answer.contains("checkpointing") || answer.contains("Checkpointing"),
        "expected answer to reference the query topic, got: {}",
        answer
    );
}

#[tokio::test]
async fn query_stream_resumes_state_for_same_thread_id() {
    let rag = setup_rag_with_docs().await;

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

    assert!(first_final.is_some(), "expected first answer");
    assert!(second_final.is_some(), "expected second answer");

    // Check turn counters
    assert!(
        first_final.unwrap().contains("#1"),
        "expected first answer to be turn #1"
    );
    assert!(
        second_final.unwrap().contains("#2"),
        "expected second answer to be turn #2"
    );
}
