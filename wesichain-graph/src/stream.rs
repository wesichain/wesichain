use crate::GraphError;
use wesichain_core::StreamEvent;

#[derive(Debug)]
pub enum GraphEvent {
    NodeEnter { node: String },
    NodeExit { node: String },
    CheckpointSaved { node: String },
    StreamEvent(StreamEvent),
    Error(GraphError),
}
