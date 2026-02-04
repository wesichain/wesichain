use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("checkpoint failed: {0}")]
    Checkpoint(String),
    #[error("node failed: {node}")]
    NodeFailed {
        node: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("missing node: {node}")]
    MissingNode { node: String },
    #[error("invalid edge to '{node}'")]
    InvalidEdge { node: String },
    #[error("Max steps exceeded: reached {reached}, limit {max}")]
    MaxStepsExceeded { max: usize, reached: usize },
    #[error("Cycle detected: node '{node}' repeated in recent window")]
    CycleDetected { node: String, recent: Vec<String> },
    #[error("interrupted")]
    Interrupted,
}
