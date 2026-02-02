use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("checkpoint failed: {0}")]
    Checkpoint(String),
    #[error("Max steps exceeded: reached {reached}, limit {max}")]
    MaxStepsExceeded { max: usize, reached: usize },
    #[error("Cycle detected: node '{node}' repeated in recent window")]
    CycleDetected { node: String, recent: Vec<String> },
}
