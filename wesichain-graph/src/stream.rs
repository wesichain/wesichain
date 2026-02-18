use crate::{GraphError, StateSchema, StateUpdate};
use wesichain_core::StreamEvent;

#[derive(Debug)]
pub enum GraphEvent<S: StateSchema> {
    NodeEnter {
        node: String,
        timestamp: u64,
    },
    NodeExit {
        node: String,
        timestamp: u64,
    },
    NodeFinished {
        node: String,
        output: String,
        timestamp: u64,
    }, // For inspection of content
    CheckpointSaved {
        node: String,
        timestamp: u64,
    },
    StateUpdate(StateUpdate<S>),
    StreamEvent(StreamEvent),
    Error(GraphError),
}
