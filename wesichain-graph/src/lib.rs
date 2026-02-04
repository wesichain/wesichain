mod checkpoint;
mod config;
mod error;
mod file_checkpointer;
mod graph;
mod interrupt;
mod program;
mod reducer;
mod state;

pub use checkpoint::{
    Checkpoint, CheckpointMetadata, Checkpointer, HistoryCheckpointer, InMemoryCheckpointer,
};
pub use config::{ExecutionConfig, ExecutionOptions};
pub use error::GraphError;
pub use file_checkpointer::{CheckpointRecord, FileCheckpointer};
pub use graph::{ExecutableGraph, GraphBuilder};
pub use interrupt::GraphInterrupt;
pub use program::{EdgeKind, GraphProgram, NodeData};
pub use reducer::{AddCounter, AppendVec, MergeMap, Override};
pub use state::{GraphState, StateReducer, StateSchema, StateUpdate};

pub const START: &str = "__start";
pub const END: &str = "__end";
