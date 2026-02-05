#[derive(Clone, Debug)]
pub struct ExecutionConfig {
    pub max_steps: Option<usize>,
    pub cycle_detection: bool,
    pub cycle_window: usize,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_steps: Some(50),
            cycle_detection: true,
            cycle_window: 20,
        }
    }
}

impl ExecutionConfig {
    pub fn merge(&self, overrides: &ExecutionOptions) -> Self {
        Self {
            max_steps: overrides.max_steps.or(self.max_steps),
            cycle_detection: overrides.cycle_detection.unwrap_or(self.cycle_detection),
            cycle_window: overrides.cycle_window.unwrap_or(self.cycle_window),
        }
    }
}

use std::sync::Arc;
use wesichain_core::callbacks::RunConfig;

use crate::Observer;

#[derive(Clone, Default)]
pub struct ExecutionOptions {
    pub max_steps: Option<usize>,
    pub cycle_detection: Option<bool>,
    pub cycle_window: Option<usize>,
    pub run_config: Option<RunConfig>,
    pub observer: Option<Arc<dyn Observer>>,
}

impl std::fmt::Debug for ExecutionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionOptions")
            .field("max_steps", &self.max_steps)
            .field("cycle_detection", &self.cycle_detection)
            .field("cycle_window", &self.cycle_window)
            .field("run_config", &self.run_config.is_some())
            .field("observer", &self.observer.is_some())
            .finish()
    }
}
