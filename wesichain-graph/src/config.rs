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

use wesichain_core::callbacks::RunConfig;

#[derive(Clone, Debug, Default)]
pub struct ExecutionOptions {
    pub max_steps: Option<usize>,
    pub cycle_detection: Option<bool>,
    pub cycle_window: Option<usize>,
    pub run_config: Option<RunConfig>,
}
