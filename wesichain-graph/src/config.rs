#[derive(Clone, Debug)]
pub struct ExecutionConfig {
    pub max_steps: Option<usize>,
    pub max_duration: Option<std::time::Duration>,
    pub node_timeout: Option<std::time::Duration>,
    pub max_visits: Option<u32>,
    pub max_loop_iterations: Option<u32>,
    pub cycle_detection: bool,
    pub cycle_window: usize,
    pub interrupt_before: Vec<String>,
    pub interrupt_after: Vec<String>,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_steps: Some(50),
            max_duration: None,
            node_timeout: None,
            max_visits: Some(10),
            max_loop_iterations: Some(15),
            cycle_detection: true,
            cycle_window: 20,
            interrupt_before: Vec::new(),
            interrupt_after: Vec::new(),
        }
    }
}

impl ExecutionConfig {
    pub fn merge(&self, overrides: &ExecutionOptions) -> Self {
        Self {
            max_steps: overrides.max_steps.or(self.max_steps),
            max_duration: overrides.max_duration.or(self.max_duration),
            node_timeout: overrides.node_timeout.or(self.node_timeout),
            max_visits: overrides.max_visits.or(self.max_visits),
            max_loop_iterations: overrides.max_loop_iterations.or(self.max_loop_iterations),
            cycle_detection: overrides.cycle_detection.unwrap_or(self.cycle_detection),
            cycle_window: overrides.cycle_window.unwrap_or(self.cycle_window),
            interrupt_before: if !overrides.interrupt_before.is_empty() {
                overrides.interrupt_before.clone()
            } else {
                self.interrupt_before.clone()
            },
            interrupt_after: if !overrides.interrupt_after.is_empty() {
                overrides.interrupt_after.clone()
            } else {
                self.interrupt_after.clone()
            },
        }
    }
}

use std::sync::Arc;
use tokio::sync::mpsc;
use wesichain_core::{AgentEvent, RunConfig};

use crate::Observer;

#[derive(Clone, Default)]
pub struct ExecutionOptions {
    pub max_steps: Option<usize>,
    pub max_duration: Option<std::time::Duration>,
    pub node_timeout: Option<std::time::Duration>,
    pub max_visits: Option<u32>,
    pub max_loop_iterations: Option<u32>,
    pub cycle_detection: Option<bool>,
    pub cycle_window: Option<usize>,
    pub interrupt_before: Vec<String>,
    pub interrupt_after: Vec<String>,
    pub initial_queue: Option<Vec<(String, u64)>>,
    pub initial_step: Option<usize>,
    pub checkpoint_thread_id: Option<String>,
    pub auto_resume: bool,
    pub run_config: Option<RunConfig>,
    pub observer: Option<Arc<dyn Observer>>,
    pub agent_event_sender: Option<mpsc::Sender<AgentEvent>>,
    pub agent_event_thread_id: Option<String>,
}

impl std::fmt::Debug for ExecutionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionOptions")
            .field("max_steps", &self.max_steps)
            .field("max_duration", &self.max_duration)
            .field("node_timeout", &self.node_timeout)
            .field("max_visits", &self.max_visits)
            .field("max_loop_iterations", &self.max_loop_iterations)
            .field("cycle_detection", &self.cycle_detection)
            .field("cycle_window", &self.cycle_window)
            .field("interrupt_before", &self.interrupt_before)
            .field("interrupt_after", &self.interrupt_after)
            // Skip queue in debug output to avoid clutter, or summarize
            .field(
                "initial_queue_len",
                &self.initial_queue.as_ref().map(|q| q.len()),
            )
            .field("checkpoint_thread_id", &self.checkpoint_thread_id)
            .field("auto_resume", &self.auto_resume)
            .field("run_config", &self.run_config.is_some())
            .field("observer", &self.observer.is_some())
            .field("agent_event_sender", &self.agent_event_sender.is_some())
            .field("agent_event_thread_id", &self.agent_event_thread_id)
            .finish()
    }
}
