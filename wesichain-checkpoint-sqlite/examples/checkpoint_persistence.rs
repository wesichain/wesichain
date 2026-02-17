use serde::{Deserialize, Serialize};
use std::error::Error;
use wesichain_checkpoint_sqlite::SqliteCheckpointer;
use wesichain_graph::Checkpointer as _;
use wesichain_graph::{Checkpoint, GraphState, StateSchema};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let checkpointer: SqliteCheckpointer = SqliteCheckpointer::builder("sqlite::memory:")
        .max_connections(5)
        .enable_projections(false)
        .build()
        .await?;

    let state = GraphState::new(DemoState { count: 0 });
    let checkpoint = Checkpoint::new(
        "example-thread".to_string(),
        state,
        1,
        "node-a".to_string(),
        vec![],
    );

    checkpointer.save(&checkpoint).await?;
    println!(
        "Checkpoint saved: thread={}, node={}, step={}",
        checkpoint.thread_id, checkpoint.node, checkpoint.step
    );

    let loaded: Option<Checkpoint<DemoState>> = checkpointer.load(&checkpoint.thread_id).await?;
    if let Some(loaded) = loaded {
        println!("\nLoaded checkpoint:");
        println!("  Thread: {}", loaded.thread_id);
        println!("  Node: {}", loaded.node);
        println!("  Step: {}", loaded.step);
        println!("  Created: {}", loaded.created_at);
        println!("  State: {:?}", loaded.state.data);
    }

    Ok(())
}
