use crate::GraphError;
use wesichain_core::StreamEvent;

#[derive(Debug)]
pub enum GraphEvent {
    NodeEnter { node: String, timestamp: u64 },
    NodeExit { node: String, timestamp: u64 },
    NodeFinished { node: String, output: String, timestamp: u64 }, // For inspection of content
    CheckpointSaved { node: String, timestamp: u64 },
    StreamEvent(StreamEvent),
    Error(GraphError),
}
