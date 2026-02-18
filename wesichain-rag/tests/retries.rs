use std::sync::{Arc, Mutex};

use futures::StreamExt;
use wesichain_core::{AgentEvent, Document, WesichainError};
use wesichain_graph::{Checkpoint, Checkpointer, InMemoryCheckpointer};
use wesichain_rag::{RagQueryRequest, RagRuntimeState, WesichainRag};

#[derive(Clone)]
struct FlakyCheckpointer {
    inner: InMemoryCheckpointer<RagRuntimeState>,
    remaining_failures: Arc<Mutex<usize>>,
}

impl FlakyCheckpointer {
    fn new(failures_before_success: usize) -> Self {
        Self {
            inner: InMemoryCheckpointer::default(),
            remaining_failures: Arc::new(Mutex::new(failures_before_success)),
        }
    }
}

#[async_trait::async_trait]
impl Checkpointer<RagRuntimeState> for FlakyCheckpointer {
    async fn save(&self, checkpoint: &Checkpoint<RagRuntimeState>) -> Result<(), WesichainError> {
        let should_fail = {
            let mut guard = self
                .remaining_failures
                .lock()
                .map_err(|_| WesichainError::Custom("retry lock poisoned".to_string()))?;

            if *guard > 0 {
                *guard -= 1;
                true
            } else {
                false
            }
        };

        if should_fail {
            return Err(WesichainError::Custom(
                "simulated transient checkpoint failure".to_string(),
            ));
        }

        self.inner.save(checkpoint).await
    }

    async fn load(
        &self,
        thread_id: &str,
    ) -> Result<Option<Checkpoint<RagRuntimeState>>, WesichainError> {
        self.inner.load(thread_id).await
    }
}

fn demo_docs() -> Vec<Document> {
    vec![Document {
        id: "retry-doc-1".to_string(),
        content: "Retries should recover from transient checkpoint failures.".to_string(),
        metadata: Default::default(),
        embedding: None,
    }]
}

#[tokio::test]
async fn recoverable_failure_retries_then_succeeds() {
    let rag = WesichainRag::builder()
        .with_checkpointer(FlakyCheckpointer::new(1))
        .with_max_retries(1)
        .build()
        .expect("rag builder should succeed");

    rag.add_documents(demo_docs())
        .await
        .expect("documents should index");

    let mut stream = rag
        .query_stream(RagQueryRequest {
            query: "Will retries recover?".to_string(),
            thread_id: Some("retry-success-thread".to_string()),
        })
        .await
        .expect("query stream should start");

    let mut events = Vec::new();
    while let Some(item) = stream.next().await {
        events.push(item.expect("stream should succeed after retry"));
    }

    assert!(
        events.iter().any(|event| matches!(
            event,
            AgentEvent::Error {
                recoverable: true,
                ..
            }
        )),
        "expected recoverable error event during retry"
    );

    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::Final { .. })),
        "expected final event after successful retry"
    );
}

#[tokio::test]
async fn exhausted_retries_returns_stream_error() {
    let rag = WesichainRag::builder()
        .with_checkpointer(FlakyCheckpointer::new(2))
        .with_max_retries(1)
        .build()
        .expect("rag builder should succeed");

    rag.add_documents(demo_docs())
        .await
        .expect("documents should index");

    let mut stream = rag
        .query_stream(RagQueryRequest {
            query: "Will retries eventually fail?".to_string(),
            thread_id: Some("retry-fail-thread".to_string()),
        })
        .await
        .expect("query stream should start");

    let mut saw_terminal_error = false;
    while let Some(item) = stream.next().await {
        match item {
            Ok(_) => {}
            Err(error) => {
                saw_terminal_error = true;
                assert!(
                    error.to_string().contains("checkpoint"),
                    "expected checkpoint-backed terminal error, got: {error}"
                );
                break;
            }
        }
    }

    assert!(
        saw_terminal_error,
        "expected stream to return terminal error when retries exhausted"
    );
}
