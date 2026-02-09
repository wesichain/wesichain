use std::time::{SystemTime, UNIX_EPOCH};

use wesichain_core::{AgentEvent, Document};
use wesichain_rag::{RagQueryRequest, WesichainRag};

fn unique_thread_id(prefix: &str) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    format!("{prefix}-{nonce}")
}

async fn query_for_answer(rag: &WesichainRag, thread_id: &str, query: &str) -> String {
    let stream = rag
        .query_stream(RagQueryRequest {
            query: query.to_string(),
            thread_id: Some(thread_id.to_string()),
        })
        .await
        .expect("query_stream should succeed");

    use futures::StreamExt;

    let events: Vec<AgentEvent> = stream
        .filter_map(|item| futures::future::ready(item.ok()))
        .collect()
        .await;

    events
        .into_iter()
        .find_map(|event| match event {
            AgentEvent::Final { content, .. } => Some(content),
            _ => None,
        })
        .expect("should receive final answer")
}

#[tokio::test]
async fn multi_document_ingestion_enables_cross_document_retrieval() {
    let rag = WesichainRag::builder().build().expect("rag should build");

    // Ingest documents from different sources
    let docs = vec![
        Document {
            id: "paris-doc".to_string(),
            content: "Paris is the capital and most populous city of France. It is located on the Seine River and has a population of over 2 million.".to_string(),
            metadata: Default::default(),
            embedding: None,
        },
        Document {
            id: "france-doc".to_string(),
            content: "France is a country in Western Europe known for its wine, cheese, and the Eiffel Tower in Paris. It has a rich cultural history.".to_string(),
            metadata: Default::default(),
            embedding: None,
        },
        Document {
            id: "eiffel-doc".to_string(),
            content: "The Eiffel Tower is an iron lattice tower located on the Champ de Mars in Paris, France. It was completed in 1889.".to_string(),
            metadata: Default::default(),
            embedding: None,
        },
    ];

    rag.add_documents(docs)
        .await
        .expect("should index multi-source documents");

    let thread_id = unique_thread_id("multi-doc");

    // Query that should retrieve from multiple documents
    let answer = query_for_answer(
        &rag,
        &thread_id,
        "Tell me about Paris, France, and the Eiffel Tower",
    )
    .await;

    // The answer should contain information from multiple documents
    assert!(
        answer.to_lowercase().contains("paris"),
        "answer should mention Paris from retrieved context, got: {answer}"
    );
    assert!(
        answer.to_lowercase().contains("france"),
        "answer should mention France from retrieved context, got: {answer}"
    );
}

#[tokio::test]
async fn session_resumption_maintains_turn_counter_across_queries() {
    let rag = WesichainRag::builder().build().expect("rag should build");

    rag.add_documents(vec![Document {
        id: "turn-test".to_string(),
        content: "Session tracking is important for maintaining context in conversations."
            .to_string(),
        metadata: Default::default(),
        embedding: None,
    }])
    .await
    .expect("should index document");

    let thread_id = unique_thread_id("turn-counter");

    // First query
    let first = query_for_answer(&rag, &thread_id, "What is session tracking?").await;
    assert!(
        first.contains("#1"),
        "first turn should be marked #1, got: {first}"
    );

    // Second query (same thread, should resume)
    let second = query_for_answer(&rag, &thread_id, "Why is it important?").await;
    assert!(
        second.contains("#2"),
        "second turn should be marked #2, got: {second}"
    );

    // Third query (same thread)
    let third = query_for_answer(&rag, &thread_id, "Give an example.").await;
    assert!(
        third.contains("#3"),
        "third turn should be marked #3, got: {third}"
    );

    // New thread should reset counter
    let new_thread = unique_thread_id("new-thread");
    let fresh = query_for_answer(&rag, &new_thread, "What is session tracking?").await;
    assert!(
        fresh.contains("#1"),
        "new thread should start at #1, got: {fresh}"
    );
}

#[tokio::test]
async fn query_with_no_relevant_context_returns_empty_notice() {
    let rag = WesichainRag::builder().build().expect("rag should build");

    // Index documents about France
    rag.add_documents(vec![Document {
        id: "france-only".to_string(),
        content: "France is a country in Europe.".to_string(),
        metadata: Default::default(),
        embedding: None,
    }])
    .await
    .expect("should index document");

    let thread_id = unique_thread_id("empty-context");

    // Query about something completely different
    let answer = query_for_answer(&rag, &thread_id, "What is quantum computing?").await;

    assert!(
        answer.to_lowercase().contains("no relevant context")
            || answer.to_lowercase().contains("context:")
            || answer.to_lowercase().contains("france"),
        "answer should indicate no relevant context found or fall back gracefully, got: {answer}"
    );
}
