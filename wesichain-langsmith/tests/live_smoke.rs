use std::env;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use secrecy::SecretString;
use wesichain_core::callbacks::{CallbackManager, RunConfig};
use wesichain_core::{Runnable, WesichainError};
use wesichain_graph::{ExecutionOptions, GraphBuilder, GraphState, StateSchema, StateUpdate};
use wesichain_langsmith::{LangSmithCallbackHandler, LangSmithConfig};

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct DemoState {
    value: usize,
}

impl StateSchema for DemoState {}

struct IncrNode;

#[async_trait::async_trait]
impl Runnable<GraphState<DemoState>, StateUpdate<DemoState>> for IncrNode {
    async fn invoke(
        &self,
        input: GraphState<DemoState>,
    ) -> Result<StateUpdate<DemoState>, WesichainError> {
        Ok(StateUpdate::new(DemoState {
            value: input.data.value + 1,
        }))
    }

    fn stream(
        &self,
        _input: GraphState<DemoState>,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
#[ignore]
async fn live_langsmith_smoke_test() {
    let api_key = env::var("LANGCHAIN_API_KEY").expect("LANGCHAIN_API_KEY must be set");
    let project = env::var("LANGSMITH_PROJECT")
        .or_else(|_| env::var("LANGCHAIN_PROJECT"))
        .unwrap_or_else(|_| "wesichain-smoke".to_string());
    let api_url = env::var("LANGCHAIN_ENDPOINT")
        .or_else(|_| env::var("LANGCHAIN_API_URL"))
        .unwrap_or_else(|_| "https://api.smith.langchain.com".to_string());

    let config = LangSmithConfig {
        api_key: SecretString::new(api_key),
        api_url,
        project_name: project,
        flush_interval: Duration::from_secs(2),
        max_batch_size: 25,
        queue_capacity: 200,
        sampling_rate: 1.0,
        redact_regex: None,
    };
    let handler = Arc::new(LangSmithCallbackHandler::new(config));
    let callbacks = CallbackManager::new(vec![handler.clone()]);

    let options = ExecutionOptions {
        run_config: Some(RunConfig {
            callbacks: Some(callbacks),
            ..Default::default()
        }),
        ..Default::default()
    };
    let graph = GraphBuilder::new()
        .add_node("first", IncrNode)
        .add_node("second", IncrNode)
        .add_edge("first", "second")
        .set_entry("first")
        .build();
    let _ = graph
        .invoke_with_options(GraphState::new(DemoState::default()), options)
        .await
        .expect("graph invoke");

    let stats = handler.flush(Duration::from_secs(10)).await.expect("flush");
    assert!(stats.events_flushed > 0);
}
