use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use secrecy::SecretString;
use serde_json::Value as JsonValue;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

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
async fn graph_invocation_posts_and_patches_runs() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let config = LangSmithConfig {
        api_key: SecretString::new("key".to_string()),
        api_url: server.uri(),
        project_name: "integration".to_string(),
        flush_interval: Duration::from_secs(3600),
        max_batch_size: 25,
        queue_capacity: 100,
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
        .unwrap();

    handler.flush(Duration::from_secs(1)).await.unwrap();

    let requests = server.received_requests().await.unwrap();
    let mut post_bodies = Vec::new();
    let mut patch_bodies = Vec::new();
    for request in requests {
        match request.method.as_str() {
            "POST" => {
                let body: JsonValue = serde_json::from_slice(&request.body).unwrap();
                post_bodies.push(body);
            }
            "PATCH" => {
                let body: JsonValue = serde_json::from_slice(&request.body).unwrap();
                patch_bodies.push(body);
            }
            _ => {}
        }
    }

    assert!(post_bodies.len() >= 3);
    assert!(patch_bodies.len() >= 3);

    let root = post_bodies
        .iter()
        .find(|body| {
            body.get("parent_run_id")
                .map(|v| v.is_null())
                .unwrap_or(false)
        })
        .expect("root run");
    let root_id = root
        .get("id")
        .and_then(|value| value.as_str())
        .expect("root id");
    let trace_id = root
        .get("trace_id")
        .and_then(|value| value.as_str())
        .expect("trace id");
    let root_order = root
        .get("dotted_order")
        .and_then(|value| value.as_str())
        .expect("dotted order");
    assert_eq!(root.get("run_type").and_then(|v| v.as_str()), Some("chain"));
    assert!(root.get("inputs").map(|v| v.is_object()).unwrap_or(false));
    assert!(root_order.ends_with(root_id));

    let child_posts: Vec<_> = post_bodies
        .iter()
        .filter(|body| body.get("parent_run_id").and_then(|v| v.as_str()) == Some(root_id))
        .collect();
    assert!(!child_posts.is_empty());
    for child in child_posts {
        assert_eq!(
            child.get("trace_id").and_then(|v| v.as_str()),
            Some(trace_id)
        );
        assert!(child.get("inputs").map(|v| v.is_object()).unwrap_or(false));
        let child_order = child
            .get("dotted_order")
            .and_then(|value| value.as_str())
            .expect("child dotted order");
        assert!(child_order.starts_with(root_order));
        let child_id = child
            .get("id")
            .and_then(|value| value.as_str())
            .expect("child id");
        assert!(child_order.ends_with(child_id));
    }

    assert!(patch_bodies
        .iter()
        .any(|body| body.get("outputs").map(|v| v.is_object()).unwrap_or(false)));
}
