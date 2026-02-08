use std::time::{SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use tempfile::NamedTempFile;
use wesichain_checkpoint_postgres::PostgresCheckpointer;
use wesichain_checkpoint_sqlite::SqliteCheckpointer;
use wesichain_core::{AgentEvent, Document};
use wesichain_rag::{RagQueryRequest, WesichainRag};

fn unique_thread_id(prefix: &str) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    format!("{prefix}-{nonce}")
}

async fn collect_final_answer(rag: &WesichainRag, thread_id: &str, query: &str) -> String {
    let mut stream = rag
        .query_stream(RagQueryRequest {
            query: query.to_string(),
            thread_id: Some(thread_id.to_string()),
        })
        .await
        .expect("query_stream should succeed");

    let mut final_answer = None;
    while let Some(event) = stream.next().await {
        match event.expect("event should be successful") {
            AgentEvent::Final { content, .. } => final_answer = Some(content),
            AgentEvent::Error { message, .. } => panic!("unexpected error event: {message}"),
            _ => {}
        }
    }

    final_answer.expect("stream should include a final answer")
}

fn demo_docs() -> Vec<Document> {
    vec![
        Document {
            id: "doc-1".to_string(),
            content: "Paris is the capital city of France.".to_string(),
            metadata: Default::default(),
            embedding: None,
        },
        Document {
            id: "doc-2".to_string(),
            content: "France is located in Western Europe.".to_string(),
            metadata: Default::default(),
            embedding: None,
        },
    ]
}

#[tokio::test]
async fn sqlite_checkpointer_supports_resume_without_api_changes() {
    let temp_db = NamedTempFile::new().expect("temporary sqlite file should be created");
    let database_url = format!("sqlite://{}", temp_db.path().display());
    let checkpointer = SqliteCheckpointer::builder(database_url)
        .max_connections(1)
        .build()
        .await
        .expect("sqlite checkpointer should build");

    let rag = WesichainRag::builder()
        .with_checkpointer(checkpointer)
        .build()
        .expect("rag facade should build with sqlite checkpointer");

    rag.add_documents(demo_docs())
        .await
        .expect("documents should be indexed");

    let thread_id = unique_thread_id("sqlite-rag");
    let first = collect_final_answer(&rag, &thread_id, "What is the capital of France?").await;
    let second = collect_final_answer(&rag, &thread_id, "Repeat that answer").await;

    assert!(first.contains("#1"), "expected first turn answer marker: {first}");
    assert!(
        second.contains("#2"),
        "expected resumed second turn answer marker: {second}"
    );
}

fn postgres_database_url() -> String {
    std::env::var("DATABASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .expect("set DATABASE_URL to run postgres integration tests")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn postgres_checkpointer_supports_resume_without_api_changes() {
    let checkpointer = PostgresCheckpointer::builder(postgres_database_url())
        .max_connections(5)
        .min_connections(1)
        .build()
        .await
        .expect("postgres checkpointer should build");

    let rag = WesichainRag::builder()
        .with_checkpointer(checkpointer)
        .build()
        .expect("rag facade should build with postgres checkpointer");

    rag.add_documents(demo_docs())
        .await
        .expect("documents should be indexed");

    let thread_id = unique_thread_id("postgres-rag");
    let first = collect_final_answer(&rag, &thread_id, "What is the capital of France?").await;
    let second = collect_final_answer(&rag, &thread_id, "Repeat that answer").await;

    assert!(first.contains("#1"), "expected first turn answer marker: {first}");
    assert!(
        second.contains("#2"),
        "expected resumed second turn answer marker: {second}"
    );
}
