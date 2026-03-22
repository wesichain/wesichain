//! Fan-out: run multiple agents concurrently and collect their results.

use std::sync::Arc;

use futures::StreamExt;
use wesichain_core::{StreamEvent, WesichainError};

use crate::supervisor::WorkerRunner;

/// Run all `agents` concurrently with the same `task` and return all outputs.
///
/// Each agent's streamed events are collected into a single string; the results
/// are returned in the same order as the input slice.
pub async fn parallel_agents(
    agents: &[Arc<dyn WorkerRunner>],
    task: impl Into<String>,
) -> Result<Vec<String>, WesichainError> {
    let task = task.into();

    let handles: Vec<_> = agents
        .iter()
        .map(|agent| {
            let runner = agent.clone();
            let t = task.clone();
            tokio::spawn(async move {
                let mut stream = runner.run(t);
                let mut buf = String::new();
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(StreamEvent::ContentChunk(s)) | Ok(StreamEvent::FinalAnswer(s)) => {
                            buf.push_str(&s);
                        }
                        _ => {}
                    }
                }
                buf
            })
        })
        .collect();

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        let output = handle.await.map_err(|e| {
            WesichainError::Custom(format!("parallel agent task panicked: {e}"))
        })?;
        results.push(output);
    }

    Ok(results)
}
