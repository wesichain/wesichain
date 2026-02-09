use std::error::Error;
use std::path::Path;

use futures::StreamExt;
use wesichain_checkpoint_sqlite::SqliteCheckpointer;
use wesichain_core::AgentEvent;
use wesichain_rag::adapters::sse::{done_event, ping_event, to_sse_event};
use wesichain_rag::{RagQueryRequest, WesichainRag};

async fn ingest_and_query(
    rag: &WesichainRag,
    query: &str,
    thread_id: &str,
) -> Result<String, Box<dyn Error>> {
    let stream = rag
        .query_stream(RagQueryRequest {
            query: query.to_string(),
            thread_id: Some(thread_id.to_string()),
        })
        .await?;

    tokio::pin!(stream);
    let mut answer = String::new();

    while let Some(item) = stream.next().await {
        match item {
            Ok(AgentEvent::Final { content, .. }) => answer = content,
            Ok(event) => print!("{}", to_sse_event(&event)),
            Err(error) => {
                print!(
                    "{}",
                    to_sse_event(&AgentEvent::Error {
                        message: error.to_string(),
                        step: 999,
                        recoverable: false,
                        source: Some("simple-rag-stream".to_string()),
                    })
                );
                break;
            }
        }
    }

    Ok(answer)
}

async fn async_main() -> Result<(), Box<dyn Error>> {
    let db_path = std::env::temp_dir().join("wesichain-rag-sessions.db");
    if !db_path.exists() {
        std::fs::File::create(&db_path)?;
    }
    let database_url = format!("sqlite://{}", db_path.display());

    let checkpointer = SqliteCheckpointer::builder(database_url)
        .max_connections(1)
        .build()
        .await?;

    let rag = WesichainRag::builder()
        .with_checkpointer(checkpointer)
        .with_max_retries(2)
        .build()?;

    // Check for fixture files
    let fixture_dir = Path::new("fixtures");
    let mut ingested_files = vec![];

    if fixture_dir.exists() {
        println!("// Discovering documents in fixtures/...");

        for entry in std::fs::read_dir(fixture_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

                match ext {
                    "txt" | "docx" | "pdf" => {
                        print!("// Ingesting: {} ... ", path.display());
                        match rag.process_file(&path).await {
                            Ok(_) => {
                                println!("OK");
                                ingested_files.push(path);
                            }
                            Err(e) => println!("ERROR: {e}"),
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Fallback: ingest inline documents if no fixture files found
    if ingested_files.is_empty() {
        println!("// No fixture files found, using inline documents...");
        rag.add_documents(vec![
            wesichain_core::Document {
                id: "demo-1".to_string(),
                content: "Paris is the capital of France. It has a population of over 2 million."
                    .to_string(),
                metadata: Default::default(),
                embedding: None,
            },
            wesichain_core::Document {
                id: "demo-2".to_string(),
                content: "France is known for its cuisine, wine, and the Eiffel Tower in Paris."
                    .to_string(),
                metadata: Default::default(),
                embedding: None,
            },
        ])
        .await?;
    }

    println!(
        "// {} documents ingested successfully\n",
        ingested_files.len().max(2)
    );

    // Demo: First query
    let thread_id = "demo-session-001";
    print!("{}", ping_event());

    println!("// Query 1: What is the capital of France?");
    let answer1 = ingest_and_query(&rag, "What is the capital of France?", thread_id).await?;
    println!(
        "// Answer: {}\n",
        answer1.lines().next().unwrap_or(&answer1)
    );

    // Demo: Follow-up query (demonstrates session resumption)
    println!("// Query 2: What else is it known for? (follow-up)");
    let answer2 = ingest_and_query(&rag, "What else is it known for?", thread_id).await?;
    println!(
        "// Answer: {}\n",
        answer2.lines().next().unwrap_or(&answer2)
    );

    // Demo: New session (should reset context)
    println!("// Query 3: What is the population? (new session)");
    let answer3 = ingest_and_query(&rag, "What is the population?", "demo-session-002").await?;
    println!(
        "// Answer: {}\n",
        answer3.lines().next().unwrap_or(&answer3)
    );

    print!("{}", done_event());

    // Resource summary
    println!("//\n// Demo complete:");
    println!("//   - Session 1: 2 turns (resumed context)");
    println!("//   - Session 2: 1 turn (fresh context)");
    println!("//   - Persistence: SQLite at {}", db_path.display());
    println!("//   - SSE events: status, trace, answer, done");

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async_main())
}
