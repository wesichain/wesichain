//! Tests for the multi-agent Supervisor.

use std::sync::{Arc, Mutex};

use futures::stream::{self, BoxStream};
use wesichain_core::{LlmRequest, LlmResponse, MessageContent, Runnable, StreamEvent, WesichainError};
use wesichain_graph::supervisor::{SupervisorBuilder, WorkerRunner, WorkerSpec};

// ── Mock LLM ─────────────────────────────────────────────────────────────────

/// Cycles through a list of response strings, one per `invoke()` call.
#[derive(Clone)]
struct SequenceLlm {
    responses: Arc<Mutex<Vec<String>>>,
    model_received: Arc<Mutex<Vec<String>>>,
}

impl SequenceLlm {
    fn new(responses: Vec<&str>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().map(String::from).rev().collect())),
            model_received: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn models_received(&self) -> Vec<String> {
        self.model_received.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl Runnable<LlmRequest, LlmResponse> for SequenceLlm {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        self.model_received.lock().unwrap().push(input.model.clone());
        let content = self.responses.lock().unwrap().pop()
            .unwrap_or_else(|| r#"{"action":"finish","answer":"done"}"#.to_string());
        Ok(LlmResponse {
            content,
            tool_calls: vec![],
            usage: None,
            model: String::new(),
        })
    }

    fn stream(&self, input: LlmRequest) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        Box::pin(stream::empty())
    }
}

// ── Mock Worker ───────────────────────────────────────────────────────────────

struct EchoWorker;

impl WorkerRunner for EchoWorker {
    fn run(&self, task: String) -> BoxStream<'static, Result<StreamEvent, WesichainError>> {
        Box::pin(stream::once(async move {
            Ok(StreamEvent::FinalAnswer(format!("worker result: {task}")))
        }))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn supervisor_finishes_immediately() {
    let llm = SequenceLlm::new(vec![r#"{"action":"finish","answer":"the answer"}"#]);
    let sup = SupervisorBuilder::new(llm).with_model("gpt-4o").build();
    let result = sup.run("what is 2+2?".to_string()).await.unwrap();
    assert_eq!(result, "the answer");
}

#[tokio::test]
async fn supervisor_delegates_to_worker() {
    // First response: delegate; second: finish
    let llm = SequenceLlm::new(vec![
        r#"{"action":"delegate","worker":"worker1","task":"sub-task"}"#,
        r#"{"action":"finish","answer":"final"}"#,
    ]);
    let sup = SupervisorBuilder::new(llm)
        .with_model("gpt-4o")
        .add_worker(WorkerSpec {
            name: "worker1".to_string(),
            description: "A test worker".to_string(),
            runner: Arc::new(EchoWorker),
        })
        .build();

    let result = sup.run("do something".to_string()).await.unwrap();
    assert_eq!(result, "final");
}

#[tokio::test]
async fn supervisor_uses_configured_model() {
    let llm = SequenceLlm::new(vec![r#"{"action":"finish","answer":"ok"}"#]);
    let models = llm.model_received.clone();
    let sup = SupervisorBuilder::new(llm).with_model("gpt-4o").build();
    let _ = sup.run("test".to_string()).await.unwrap();

    let received = models.lock().unwrap();
    assert_eq!(received[0], "gpt-4o", "supervisor should send configured model name to LLM");
}

#[tokio::test]
async fn supervisor_respects_max_rounds() {
    // LLM always delegates — should hit max_rounds limit
    let responses: Vec<&str> = vec![
        r#"{"action":"delegate","worker":"worker1","task":"t"}"#;
        20
    ];
    let llm = SequenceLlm::new(responses);
    let sup = SupervisorBuilder::new(llm)
        .with_model("gpt-4o")
        .max_rounds(3)
        .add_worker(WorkerSpec {
            name: "worker1".to_string(),
            description: "Loops forever".to_string(),
            runner: Arc::new(EchoWorker),
        })
        .build();

    let err = sup.run("infinite loop".to_string()).await.unwrap_err();
    assert!(
        err.to_string().contains("max_rounds"),
        "expected max_rounds error, got: {err}"
    );
}
