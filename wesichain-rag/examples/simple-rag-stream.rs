use std::error::Error;

use futures::StreamExt;
use wesichain_checkpoint_sqlite::SqliteCheckpointer;
use wesichain_core::AgentEvent;
use wesichain_rag::adapters::sse::{done_event, ping_event, to_sse_event};
use wesichain_rag::{RagQueryRequest, WesichainRag};

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
        .build()?;

    let mut stream = rag
        .query_stream(RagQueryRequest {
            query: "What is the capital of France?".to_string(),
            thread_id: Some("simple-rag-stream-demo".to_string()),
        })
        .await?;

    print!("{}", ping_event());

    while let Some(item) = stream.next().await {
        match item {
            Ok(event) => {
                print!("{}", to_sse_event(&event));
            }
            Err(error) => {
                let event = AgentEvent::Error {
                    message: error.to_string(),
                    step: 999,
                    recoverable: false,
                    source: Some("simple-rag-stream".to_string()),
                };
                print!("{}", to_sse_event(&event));
                break;
            }
        }
    }

    print!("{}", done_event());
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async_main())
}
