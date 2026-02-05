use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("checkpoint failed: {0}")]
    Checkpoint(String),
    #[error("node failed: {node}: {source}")]
    NodeFailed {
        node: String,
        #[source]
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
    #[error("tool call failed for '{0}': {1}")]
    ToolCallFailed(String, String),
    #[error("invalid tool call response: {0}")]
    InvalidToolCallResponse(String),
    #[error("duplicate tool name: {0}")]
    DuplicateToolName(String),
}
