//! Supervisor + worker multi-agent orchestration.
//!
//! The [`SupervisorBuilder`] creates a supervisor agent that delegates tasks to
//! specialized workers based on LLM-driven routing decisions.
//!
//! # Example
//! ```ignore
//! use wesichain_graph::supervisor::{SupervisorBuilder, WorkerSpec};
//! use wesichain_agent::as_tool::AgentAsTool;
//!
//! let supervisor = SupervisorBuilder::new(llm)
//!     .add_worker(WorkerSpec {
//!         name: "researcher".to_string(),
//!         description: "Searches the web and summarises findings".to_string(),
//!         runner: Arc::new(researcher_agent),
//!     })
//!     .add_worker(WorkerSpec {
//!         name: "coder".to_string(),
//!         description: "Writes and reviews Rust code".to_string(),
//!         runner: Arc::new(coder_agent),
//!     })
//!     .build();
//!
//! let result = supervisor.run("Implement and document a binary search function".to_string()).await?;
//! ```

use std::sync::Arc;

use futures::stream::BoxStream;
use serde::Deserialize;
use wesichain_core::{
    LlmRequest, LlmResponse, Message, MessageContent, Runnable, StreamEvent, WesichainError,
};

// ── WorkerRunner ──────────────────────────────────────────────────────────────

/// An agent or callable that accepts a task string and returns a streaming response.
pub trait WorkerRunner: Send + Sync {
    fn run(&self, task: String) -> BoxStream<'static, Result<StreamEvent, WesichainError>>;
}

// ── WorkerSpec ────────────────────────────────────────────────────────────────

/// Description of a worker that the supervisor can delegate to.
pub struct WorkerSpec {
    /// Short unique name used by the LLM to refer to this worker.
    pub name: String,
    /// Human-readable description the supervisor LLM uses to decide routing.
    pub description: String,
    /// The actual worker implementation.
    pub runner: Arc<dyn WorkerRunner>,
}

// ── SupervisorBuilder ─────────────────────────────────────────────────────────

/// Builds a [`Supervisor`] by registering workers and an LLM router.
pub struct SupervisorBuilder<L> {
    llm: L,
    workers: Vec<WorkerSpec>,
    max_rounds: usize,
    model: String,
}

impl<L> SupervisorBuilder<L>
where
    L: Runnable<LlmRequest, LlmResponse> + Clone + Send + Sync + 'static,
{
    pub fn new(llm: L) -> Self {
        Self { llm, workers: Vec::new(), max_rounds: 10, model: String::new() }
    }

    pub fn add_worker(mut self, worker: WorkerSpec) -> Self {
        self.workers.push(worker);
        self
    }

    /// Maximum supervisor→worker delegation rounds before forcing a final answer.
    pub fn max_rounds(mut self, max: usize) -> Self {
        self.max_rounds = max;
        self
    }

    /// Set the model name sent to the LLM provider (e.g. `"gpt-4o"`).
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn build(self) -> Supervisor<L> {
        Supervisor {
            llm: self.llm,
            workers: Arc::new(self.workers),
            max_rounds: self.max_rounds,
            model: self.model,
        }
    }
}

// ── Supervisor ────────────────────────────────────────────────────────────────

/// Multi-agent supervisor that delegates tasks to specialized workers.
pub struct Supervisor<L> {
    llm: L,
    workers: Arc<Vec<WorkerSpec>>,
    max_rounds: usize,
    model: String,
}

impl<L> Supervisor<L>
where
    L: Runnable<LlmRequest, LlmResponse> + Clone + Send + Sync + 'static,
{
    /// Run the supervisor loop and return the final answer string.
    pub async fn run(&self, task: String) -> Result<String, WesichainError> {
        let worker_list = self
            .workers
            .iter()
            .map(|w| format!("- {}: {}", w.name, w.description))
            .collect::<Vec<_>>()
            .join("\n");

        let system_prompt = format!(
            "You are a supervisor agent. You have access to the following workers:\n\
             {worker_list}\n\n\
             To delegate a task respond ONLY with JSON: \
             {{\"action\":\"delegate\",\"worker\":\"<name>\",\"task\":\"<task>\"}}\n\
             When you have a final answer respond ONLY with JSON: \
             {{\"action\":\"finish\",\"answer\":\"<answer>\"}}"
        );

        let mut messages = vec![
            Message { role: wesichain_core::Role::System, content: MessageContent::Text(system_prompt), tool_call_id: None, tool_calls: vec![] },
            Message { role: wesichain_core::Role::User, content: MessageContent::Text(task.clone()), tool_call_id: None, tool_calls: vec![] },
        ];

        for _ in 0..self.max_rounds {
            let req = LlmRequest {
                model: self.model.clone(),
                messages: messages.clone(),
                tools: vec![],
                temperature: Some(0.0),
                max_tokens: None,
                stop_sequences: vec![],
            };

            let resp = self.llm.invoke(req).await?;
            let text = resp.content.trim().to_string();

            // Parse supervisor decision
            match serde_json::from_str::<SupervisorDecision>(&text) {
                Ok(SupervisorDecision::Delegate { worker, task: sub_task }) => {
                    // Find the worker
                    let runner = self
                        .workers
                        .iter()
                        .find(|w| w.name == worker)
                        .map(|w| w.runner.clone());

                    let worker_output = if let Some(runner) = runner {
                        collect_stream_to_text(runner.run(sub_task)).await
                    } else {
                        format!("[error: unknown worker '{worker}']")
                    };

                    // Feed result back to supervisor
                    messages.push(Message {
                        role: wesichain_core::Role::Assistant,
                        content: MessageContent::Text(text),
                        tool_call_id: None,
                        tool_calls: vec![],
                    });
                    messages.push(Message {
                        role: wesichain_core::Role::User,
                        content: MessageContent::Text(format!(
                            "Worker '{worker}' returned:\n{worker_output}\n\nContinue."
                        )),
                        tool_call_id: None,
                        tool_calls: vec![],
                    });
                }
                Ok(SupervisorDecision::Finish { answer }) => {
                    return Ok(answer);
                }
                Err(_) => {
                    // LLM produced free text — treat as final answer
                    return Ok(text);
                }
            }
        }

        Err(WesichainError::Custom(format!(
            "Supervisor exceeded max_rounds ({}) without finishing",
            self.max_rounds
        )))
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
enum SupervisorDecision {
    Delegate { worker: String, task: String },
    Finish { answer: String },
}

async fn collect_stream_to_text(
    mut stream: BoxStream<'static, Result<StreamEvent, WesichainError>>,
) -> String {
    use futures::StreamExt;
    let mut buf = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(StreamEvent::ContentChunk(s)) | Ok(StreamEvent::FinalAnswer(s)) => buf.push_str(&s),
            _ => {}
        }
    }
    buf
}
